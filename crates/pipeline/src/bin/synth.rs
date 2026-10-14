//! Gera um .pcap sintético de uma luta em grupo, no layout de pacotes do TK.
//! Serve para desenvolver a interface sem estar no jogo; não é tráfego real.
//!
//! Uso: `cargo run -p pipeline --bin synth -- fixtures/sintetico-luta-grupo.pcap`

use std::net::SocketAddrV4;

use capture::replay::Recorder;
use capture::segment::build_ethernet_frame;
use capture::LinkType;
use protocol::framing::{build_compressed_frame, build_frame};
use protocol::varint::encode;

fn damage(target: u32, actor: u32, skill: u32, dmg: u32, perfect: bool) -> Vec<u8> {
    let mut b = vec![0x04, 0x38];
    encode(target, &mut b);
    encode(6, &mut b); // switch -> bloco de 10 bytes com flags
    encode(0, &mut b);
    encode(actor, &mut b);
    b.extend_from_slice(&skill.to_le_bytes());
    b.push(0);
    encode(1, &mut b);
    let mut flags = [0u8; 10];
    flags[0] = if perfect { 0x08 } else { 0 };
    b.extend_from_slice(&flags);
    encode(0, &mut b);
    encode(dmg, &mut b);
    b.extend_from_slice(&[0, 0, 0]);
    build_frame(None, &b)
}

fn name(op: u8, entity: u32, name: &str, job: u8) -> Vec<u8> {
    let mut b = vec![op, 0x36];
    encode(entity, &mut b);
    if op == 0x33 {
        b.push(0x07);
    } else {
        encode(0, &mut b);
        encode(0, &mut b);
        b.push(0);
    }
    encode(name.len() as u32, &mut b);
    b.extend_from_slice(name.as_bytes());
    if op == 0x33 {
        b.extend_from_slice(&1u16.to_le_bytes());
    }
    b.push(job);
    build_frame(None, &b)
}

fn main() {
    let out = std::env::args().nth(1).unwrap_or_else(|| "sintetico.pcap".into());
    let server: SocketAddrV4 = "203.0.113.10:13328".parse().unwrap();
    let client: SocketAddrV4 = "192.168.1.20:51000".parse().unwrap();
    let mut rec = Recorder::create(out.as_ref(), LinkType::Ethernet).expect("criar pcap");
    let mut seq = 1_000u32;
    let mut ts = 1_760_000_000_000u64;
    let mut send = |rec: &mut Recorder, ts: u64, payload: Vec<u8>| {
        rec.write(ts, &build_ethernet_frame(server, client, seq, &payload))
            .unwrap();
        seq = seq.wrapping_add(payload.len() as u32);
    };

    send(
        &mut rec,
        ts,
        [
            name(0x33, 1, "Giovanni", 26),
            name(0x44, 2, "Lâmina", 6),
            name(0x44, 3, "Flecha", 14),
            name(0x44, 4, "Prece", 30),
        ]
        .concat(),
    );

    // (ator, skills, dano base)
    let party: [(u32, [u32; 3], u32); 4] = [
        (1, [15_010_010, 15_020_010, 15_210_010], 2_400),
        (2, [11_020_010, 11_030_010, 11_040_010], 2_100),
        (3, [14_020_010, 14_030_010, 14_050_010], 1_900),
        (4, [17_010_010, 17_020_010, 17_030_010], 600),
    ];
    let boss = 9_001;
    let mut rng = 12_345u32;
    let mut next = || {
        rng ^= rng << 13;
        rng ^= rng >> 17;
        rng ^= rng << 5;
        rng
    };
    for tick in 0..60 {
        ts += 500;
        let mut frames = Vec::new();
        for (actor, skills, base) in &party {
            let skill = skills[(next() % 3) as usize];
            let dmg = base + next() % base;
            frames.push(damage(boss, *actor, skill, dmg, next() % 7 == 0));
        }
        let payload = if tick % 3 == 0 {
            build_compressed_frame(&frames)
        } else {
            frames.concat()
        };
        send(&mut rec, ts, payload);
    }
    println!("gravado em {out}");
}
