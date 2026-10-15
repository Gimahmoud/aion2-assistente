//! Imprime cada evento decodificado de um .pcap (para investigar o protocolo).
//!
//! Uso: `cargo run -p pipeline --bin dump -- gravacao.pcap [--hex] [--strings]`
//! `--strings` lista textos (nomes) por opcode; com `DUMP_HEX=<texto>` mostra
//! os bytes das mensagens que contêm aquele texto.

use capture::{flows::FlowTable, replay, ServerFilter};
use protocol::{Decoder, GameEvent, OpcodeTable};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(path) = args.iter().find(|a| !a.starts_with("--")) else {
        eprintln!("uso: dump <arquivo.pcap> [--hex]");
        std::process::exit(2);
    };
    let hex = args.iter().any(|a| a == "--hex");
    let strings = args.iter().any(|a| a == "--strings");
    let segs = replay::read_segments(path.as_ref()).unwrap_or_else(|e| {
        eprintln!("erro ao ler {path}: {e}");
        std::process::exit(1);
    });
    let mut flows = FlowTable::new(ServerFilter::LowerPort);
    let mut dec = Decoder::new(OpcodeTable::default());
    let t0 = segs.first().map_or(0, |s| s.ts_ms);
    for s in &segs {
        for m in flows.push(s) {
            if strings {
                print_strings(&m);
            }
            for ev in dec.decode(&m) {
                let t = (s.ts_ms - t0) as f64 / 1000.0;
                println!("{t:8.3} {}", serde_json::to_string(&ev).unwrap());
                if hex
                    && matches!(
                        ev,
                        GameEvent::Damage(_) | GameEvent::OwnCharacter { .. } | GameEvent::MobHp { .. }
                    )
                {
                    let h: Vec<String> = m.bytes.iter().map(|b| format!("{b:02x}")).collect();
                    println!("         {}", h.join(" "));
                }
            }
        }
    }
}

/// Trechos de texto (nomes) dentro de qualquer mensagem, com o opcode.
fn print_strings(m: &protocol::Message) {
    let b = &m.bytes;
    let mut i = 0;
    while i < b.len() {
        let len = b[i] as usize;
        if (3..=24).contains(&len) && i + 1 + len <= b.len() {
            if let Ok(s) = std::str::from_utf8(&b[i + 1..i + 1 + len]) {
                if s.chars().all(char::is_alphanumeric) && s.chars().filter(|c| c.is_alphabetic()).count() >= 3 {
                    println!("str {:#06x} @{i} {s}", m.opcode().unwrap_or(0));
                    if std::env::var("DUMP_HEX").is_ok_and(|v| v == s) {
                        let h: Vec<String> = b.iter().map(|x| format!("{x:02x}")).collect();
                        println!("    {}", h.join(" "));
                    }
                    i += 1 + len;
                    continue;
                }
            }
        }
        i += 1;
    }
}
