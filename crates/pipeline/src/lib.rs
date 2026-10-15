//! Segmento TCP -> mensagens -> eventos -> lutas.
//!
//! Não conhece Tauri nem threads: quem chama decide de onde vêm os segmentos
//! (captura ao vivo ou replay) e o que fazer com as lutas encerradas.

use std::collections::HashMap;

use capture::{FlowStats, FlowTable, Segment, ServerFilter};
use combat::{Class, EncounterSnapshot, Engine, EngineConfig};
use protocol::{Decoder, GameEvent, OpcodeTable};
use serde::Serialize;

#[derive(Debug, Clone, Default, Serialize)]
pub struct PipelineStats {
    pub segments: u64,
    pub bytes: u64,
    pub messages: u64,
    pub compressed_batches: u64,
    pub resyncs: u64,
    pub tcp_gaps: u64,
    pub decompress_errors: u64,
    pub events: HashMap<&'static str, u64>,
    /// Opcodes sem decodificador, como "0x3804", com contagem. Top 20.
    pub unknown_opcodes: Vec<(String, u64)>,
    pub opcode_table: String,
}

/// Personagem logado. Vem do pacote de entrada no mundo ou, no SA, do pacote
/// do grupo; quando o pacote não traz a classe, ela é deduzida das skills.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct OwnCharacter {
    pub name: String,
    pub server: Option<u16>,
    pub class: Option<Class>,
}

pub struct Pipeline {
    flows: FlowTable,
    decoder: Decoder,
    engine: Engine,
    events: HashMap<&'static str, u64>,
    own: Option<OwnCharacter>,
}

impl Pipeline {
    pub fn new(table: OpcodeTable, filter: ServerFilter, cfg: EngineConfig) -> Self {
        Self {
            flows: FlowTable::new(filter),
            decoder: Decoder::new(table),
            engine: Engine::new(cfg),
            events: HashMap::new(),
            own: None,
        }
    }

    pub fn set_filter(&mut self, filter: ServerFilter) {
        self.flows.set_filter(filter);
    }

    /// Processa um segmento. Devolve as lutas que se encerraram.
    pub fn push_segment(&mut self, seg: &Segment) -> Vec<EncounterSnapshot> {
        let mut closed = Vec::new();
        for msg in self.flows.push(seg) {
            for ev in self.decoder.decode(&msg) {
                *self.events.entry(kind(&ev)).or_default() += 1;
                if let GameEvent::OwnCharacter {
                    name, server, job_code, ..
                } = &ev
                {
                    let class = job_code.and_then(Class::from_job_code).or_else(|| {
                        // Mesmo personagem visto antes: mantém a classe conhecida.
                        self.own.as_ref().filter(|o| &o.name == name).and_then(|o| o.class)
                    });
                    self.own = Some(OwnCharacter {
                        name: name.clone(),
                        server: *server,
                        class,
                    });
                }
                closed.extend(self.engine.handle(&ev, seg.ts_ms));
                if let Some(own) = self.own.as_mut().filter(|o| o.class.is_none()) {
                    own.class = self.engine.self_id().and_then(|id| self.engine.class_of(id));
                }
            }
        }
        closed
    }

    pub fn tick(&mut self, now_ms: u64) -> Option<EncounterSnapshot> {
        self.engine.tick(now_ms)
    }

    pub fn reset(&mut self) -> Option<EncounterSnapshot> {
        self.engine.reset()
    }

    pub fn snapshot(&self) -> Option<EncounterSnapshot> {
        self.engine.snapshot()
    }

    /// Último personagem logado visto no tráfego.
    pub fn own_character(&self) -> Option<&OwnCharacter> {
        self.own.as_ref()
    }

    pub fn opcode_version(&self) -> &str {
        &self.decoder.table().version
    }

    pub fn stats(&self) -> PipelineStats {
        let FlowStats {
            segments,
            bytes,
            tcp_gaps,
            framing,
        } = self.flows.stats();
        let mut unknown: Vec<(String, u64)> = self
            .decoder
            .unknown_opcodes
            .iter()
            .map(|(op, n)| (format!("0x{op:04x}"), *n))
            .collect();
        unknown.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        unknown.truncate(20);
        PipelineStats {
            segments,
            bytes,
            messages: framing.messages,
            compressed_batches: framing.compressed_batches,
            resyncs: framing.resyncs,
            tcp_gaps,
            decompress_errors: framing.decompress_errors,
            events: {
                let mut ev = self.events.clone();
                ev.insert("damage_ignored", self.engine.ignored_damage());
                ev
            },
            unknown_opcodes: unknown,
            opcode_table: self.decoder.table().version.clone(),
        }
    }
}

fn kind(ev: &GameEvent) -> &'static str {
    match ev {
        GameEvent::Damage(d) if d.dot => "dot",
        GameEvent::Damage(_) => "damage",
        GameEvent::OwnCharacter { .. } => "own_character",
        GameEvent::OtherCharacter { .. } => "other_character",
        GameEvent::MobSpawn { .. } => "mob_spawn",
        GameEvent::SummonOwner { .. } => "summon_owner",
        GameEvent::BattleToggle { .. } => "battle_toggle",
        GameEvent::MobHp { .. } => "mob_hp",
    }
}
