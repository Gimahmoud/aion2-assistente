//! Do quadro Ethernet até o DPS, com pacotes sintéticos no layout do TK.
//! Não substitui testes com gravações reais do servidor global (M2).

use capture::segment::{build_ethernet_frame, parse_frame};
use capture::{LinkType, ServerFilter};
use combat::EngineConfig;
use pipeline::Pipeline;
use protocol::framing::{build_compressed_frame, build_frame};
use protocol::varint::encode;
use protocol::OpcodeTable;

fn damage(target: u32, actor: u32, skill: u32, dmg: u32) -> Vec<u8> {
    let mut b = vec![0x04, 0x38];
    encode(target, &mut b);
    encode(4, &mut b); // switch -> bloco de 8 bytes
    encode(0, &mut b);
    encode(actor, &mut b);
    b.extend_from_slice(&skill.to_le_bytes());
    b.push(0);
    encode(1, &mut b);
    b.extend_from_slice(&[0u8; 8]);
    encode(0, &mut b);
    encode(dmg, &mut b);
    b.extend_from_slice(&[0, 0, 0]);
    build_frame(None, &b)
}

fn own_name(entity: u32, name: &str) -> Vec<u8> {
    let mut b = vec![0x33, 0x36];
    encode(entity, &mut b);
    b.push(0x07);
    encode(name.len() as u32, &mut b);
    b.extend_from_slice(name.as_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.push(26);
    build_frame(None, &b)
}

#[test]
fn stream_to_dps() {
    let server = "203.0.113.10:13328".parse().unwrap();
    let client = "192.168.1.20:51000".parse().unwrap();

    // Stream: nome, dano solto, lote comprimido com dois danos.
    let stream = [
        own_name(7, "Giovanni"),
        damage(900, 7, 15_010_010, 1_000),
        build_compressed_frame(&[damage(900, 7, 15_020_010, 2_000), damage(900, 8, 11_020_010, 500)]),
    ]
    .concat();

    // Quebra em segmentos de 13 bytes, entregues fora de ordem.
    let mut segs: Vec<_> = stream
        .chunks(13)
        .scan(1_000u32, |seq, chunk| {
            let s = *seq;
            *seq += chunk.len() as u32;
            Some((s, chunk.to_vec()))
        })
        .collect();
    segs.swap(1, 2);

    let mut p = Pipeline::new(OpcodeTable::default(), ServerFilter::LowerPort, EngineConfig::default());
    for (i, (seq, payload)) in segs.iter().enumerate() {
        let frame = build_ethernet_frame(server, client, *seq, payload);
        let seg = parse_frame(LinkType::Ethernet, &frame, 10_000 + i as u64 * 1_000).unwrap();
        assert!(p.push_segment(&seg).is_empty());
    }

    let snap = p.snapshot().expect("luta em andamento");
    assert_eq!(snap.total_damage, 3_500);
    let me = snap.actors.iter().find(|a| a.is_self).unwrap();
    assert_eq!(me.name, "Giovanni");
    assert_eq!(me.damage, 3_000);
    assert_eq!(me.skills.len(), 2);

    let stats = p.stats();
    assert_eq!(stats.compressed_batches, 1);
    assert_eq!(stats.events["damage"], 3);
    assert_eq!(stats.resyncs, 0);

    let closed = p.tick(snap.started_at_ms + 60_000).unwrap();
    assert!(!closed.live);
}
