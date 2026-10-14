//! Processa um .pcap gravado e imprime as lutas encontradas.
//!
//! Uso: `cargo run -p pipeline --bin replay -- gravacao.pcap [--json]`

use capture::{replay, ServerFilter};
use combat::{EncounterSnapshot, EngineConfig};
use pipeline::Pipeline;
use protocol::OpcodeTable;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(path) = args.iter().find(|a| !a.starts_with("--")) else {
        eprintln!("uso: replay <arquivo.pcap> [--json]");
        std::process::exit(2);
    };
    let json = args.iter().any(|a| a == "--json");

    let segs = match replay::read_segments(path.as_ref()) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("erro ao ler {path}: {e}");
            std::process::exit(1);
        }
    };
    let mut p = Pipeline::new(OpcodeTable::default(), ServerFilter::LowerPort, EngineConfig::default());
    let mut fights: Vec<EncounterSnapshot> = Vec::new();
    for s in &segs {
        fights.extend(p.push_segment(s));
    }
    fights.extend(p.reset());

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({ "stats": p.stats(), "encounters": fights })).unwrap()
        );
        return;
    }
    println!("{}", serde_json::to_string_pretty(&p.stats()).unwrap());
    for f in &fights {
        println!(
            "\nLuta #{}  {:.1}s  dano total {}  alvo {:?}",
            f.id,
            f.duration_ms as f64 / 1000.0,
            f.total_damage,
            f.target.as_ref().map(|t| t.mob_code.unwrap_or(t.entity))
        );
        for a in &f.actors {
            println!(
                "  {:<16} {:>12} dano  {:>10.0} DPS  {:>5.1}%  {:?}",
                a.name,
                a.damage,
                a.dps,
                a.share * 100.0,
                a.class
            );
        }
    }
}
