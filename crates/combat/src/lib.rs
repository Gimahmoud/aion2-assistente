//! Motor de combate. Recebe [`GameEvent`]s e mantém a luta atual.
//!
//! Uma luta começa no primeiro dano de um jogador e termina depois de
//! `idle_timeout_ms` sem dano, quando o alvo principal sai de combate ou
//! quando o usuário reseta.

pub mod class;

use std::collections::HashMap;

use protocol::{DamageEvent, EntityId, GameEvent};
use serde::{Deserialize, Serialize};

pub use class::{normalize_skill, Class};

#[derive(Debug, Clone)]
pub struct EngineConfig {
    pub idle_timeout_ms: u64,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self { idle_timeout_ms: 8_000 }
    }
}

#[derive(Debug, Clone, Default)]
struct Entity {
    name: Option<String>,
    class: Option<Class>,
    is_player: bool,
}

#[derive(Debug, Clone, Default)]
struct SkillAcc {
    damage: u64,
    hits: u64,
    max_hit: u64,
    dot: bool,
}

#[derive(Debug, Clone, Default)]
struct ActorAcc {
    damage: u64,
    hits: u64,
    max_hit: u64,
    perfect: u64,
    back: u64,
    skills: HashMap<u32, SkillAcc>,
}

#[derive(Debug, Clone)]
struct Encounter {
    id: u64,
    start_ms: u64,
    last_ms: u64,
    actors: HashMap<EntityId, ActorAcc>,
    damage_by_target: HashMap<EntityId, u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SkillStats {
    pub skill: u32,
    pub damage: u64,
    pub hits: u64,
    pub max_hit: u64,
    pub dot: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActorStats {
    pub entity: EntityId,
    pub name: String,
    pub class: Option<Class>,
    pub is_self: bool,
    pub damage: u64,
    pub dps: f64,
    /// Fração do dano total da luta, 0..=1.
    pub share: f64,
    pub hits: u64,
    pub max_hit: u64,
    pub perfect_hits: u64,
    pub back_hits: u64,
    pub skills: Vec<SkillStats>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TargetInfo {
    pub entity: EntityId,
    pub mob_code: Option<u32>,
    pub hp: Option<u32>,
}

/// Retrato de uma luta, ao vivo ou encerrada.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EncounterSnapshot {
    pub id: u64,
    pub started_at_ms: u64,
    pub duration_ms: u64,
    pub total_damage: u64,
    pub target: Option<TargetInfo>,
    pub actors: Vec<ActorStats>,
    pub live: bool,
}

#[derive(Default)]
pub struct Engine {
    cfg: EngineConfig,
    entities: HashMap<EntityId, Entity>,
    summon_owner: HashMap<EntityId, EntityId>,
    mob_code: HashMap<EntityId, u32>,
    mob_hp: HashMap<EntityId, u32>,
    self_id: Option<EntityId>,
    current: Option<Encounter>,
    next_id: u64,
    ignored_damage: u64,
}

impl Engine {
    pub fn new(cfg: EngineConfig) -> Self {
        Self {
            cfg,
            next_id: 1,
            ..Default::default()
        }
    }

    /// Processa um evento. Devolve a luta anterior se este evento a encerrou.
    pub fn handle(&mut self, ev: &GameEvent, ts_ms: u64) -> Option<EncounterSnapshot> {
        let mut closed = self.tick(ts_ms);
        match ev {
            GameEvent::Damage(d) => self.on_damage(d, ts_ms),
            GameEvent::OwnCharacter {
                entity, name, job_code, ..
            } => {
                self.self_id = Some(*entity);
                self.set_player(*entity, name, *job_code);
            }
            GameEvent::OtherCharacter { entity, name, job_code } => self.set_player(*entity, name, *job_code),
            GameEvent::MobSpawn { entity, mob_code } => {
                self.mob_code.insert(*entity, *mob_code);
            }
            GameEvent::SummonOwner { summon, owner } => {
                self.summon_owner.insert(*summon, *owner);
            }
            GameEvent::MobHp { entity, hp } => {
                self.mob_hp.insert(*entity, *hp);
            }
            GameEvent::BattleToggle {
                entity,
                in_battle: false,
            } => {
                if self.current.as_ref().and_then(main_target) == Some(*entity) {
                    closed = closed.or_else(|| self.close());
                }
            }
            GameEvent::BattleToggle { .. } => {}
        }
        closed
    }

    /// Encerra a luta se passou do tempo sem dano.
    pub fn tick(&mut self, now_ms: u64) -> Option<EncounterSnapshot> {
        let idle = self
            .current
            .as_ref()
            .is_some_and(|e| now_ms.saturating_sub(e.last_ms) > self.cfg.idle_timeout_ms);
        if idle {
            self.close()
        } else {
            None
        }
    }

    /// Encerra a luta atual por pedido do usuário.
    pub fn reset(&mut self) -> Option<EncounterSnapshot> {
        self.close()
    }

    pub fn snapshot(&self) -> Option<EncounterSnapshot> {
        self.current.as_ref().map(|e| self.build(e, true))
    }

    /// Danos decodificados mas descartados (atacante não reconhecido como
    /// jogador ou alvo jogador). Aparece no Diagnóstico.
    pub fn ignored_damage(&self) -> u64 {
        self.ignored_damage
    }

    pub fn self_id(&self) -> Option<EntityId> {
        self.self_id
    }

    fn close(&mut self) -> Option<EncounterSnapshot> {
        let e = self.current.take()?;
        Some(self.build(&e, false))
    }

    fn set_player(&mut self, id: EntityId, name: &str, job: Option<u8>) {
        let e = self.entities.entry(id).or_default();
        e.name = Some(name.to_owned());
        e.is_player = true;
        if let Some(c) = job.and_then(Class::from_job_code) {
            e.class = Some(c);
        }
    }

    /// Dono final de uma entidade (invocações somam ao invocador).
    fn owner_of(&self, mut id: EntityId) -> EntityId {
        for _ in 0..4 {
            match self.summon_owner.get(&id) {
                Some(&o) if o != id => id = o,
                _ => break,
            }
        }
        id
    }

    fn on_damage(&mut self, d: &DamageEvent, ts: u64) {
        let actor = self.owner_of(d.actor);
        let skill = normalize_skill(d.skill_raw);
        let skill_class = Class::from_skill(skill);
        let known_player = self.entities.get(&actor).is_some_and(|e| e.is_player);
        // Só conta dano causado por jogadores; monstros batendo em nós não.
        if !known_player && skill_class.is_none() {
            self.ignored_damage += 1;
            return;
        }
        if self.entities.get(&d.target).is_some_and(|e| e.is_player) {
            self.ignored_damage += 1;
            return;
        }
        let ent = self.entities.entry(actor).or_default();
        if ent.class.is_none() {
            ent.class = skill_class;
        }

        let id = &mut self.next_id;
        let enc = self.current.get_or_insert_with(|| {
            let e = Encounter {
                id: *id,
                start_ms: ts,
                last_ms: ts,
                actors: HashMap::new(),
                damage_by_target: HashMap::new(),
            };
            *id += 1;
            e
        });
        enc.last_ms = enc.last_ms.max(ts);

        let hits = 1;
        let total = u64::from(d.damage);
        let a = enc.actors.entry(actor).or_default();
        a.damage += total;
        a.hits += hits;
        a.max_hit = a.max_hit.max(u64::from(d.damage));
        a.perfect += u64::from(d.specials.perfect);
        a.back += u64::from(d.specials.back);
        let s = a.skills.entry(skill).or_default();
        s.damage += total;
        s.hits += hits;
        s.max_hit = s.max_hit.max(u64::from(d.damage));
        s.dot |= d.dot;
        *enc.damage_by_target.entry(d.target).or_default() += total;
    }

    fn build(&self, e: &Encounter, live: bool) -> EncounterSnapshot {
        let duration_ms = e.last_ms - e.start_ms;
        let secs = (duration_ms.max(1_000)) as f64 / 1_000.0;
        let total: u64 = e.actors.values().map(|a| a.damage).sum();
        let mut actors: Vec<ActorStats> = e
            .actors
            .iter()
            .map(|(&id, a)| {
                let ent = self.entities.get(&id);
                let mut skills: Vec<SkillStats> = a
                    .skills
                    .iter()
                    .map(|(&skill, s)| SkillStats {
                        skill,
                        damage: s.damage,
                        hits: s.hits,
                        max_hit: s.max_hit,
                        dot: s.dot,
                    })
                    .collect();
                skills.sort_by(|x, y| y.damage.cmp(&x.damage).then(x.skill.cmp(&y.skill)));
                ActorStats {
                    entity: id,
                    name: ent.and_then(|x| x.name.clone()).unwrap_or_else(|| format!("#{id}")),
                    class: ent.and_then(|x| x.class),
                    is_self: self.self_id == Some(id),
                    damage: a.damage,
                    dps: a.damage as f64 / secs,
                    share: if total > 0 { a.damage as f64 / total as f64 } else { 0.0 },
                    hits: a.hits,
                    max_hit: a.max_hit,
                    perfect_hits: a.perfect,
                    back_hits: a.back,
                    skills,
                }
            })
            .collect();
        actors.sort_by(|x, y| y.damage.cmp(&x.damage).then(x.entity.cmp(&y.entity)));
        let target = main_target(e).map(|t| TargetInfo {
            entity: t,
            mob_code: self.mob_code.get(&t).copied(),
            hp: self.mob_hp.get(&t).copied(),
        });
        EncounterSnapshot {
            id: e.id,
            started_at_ms: e.start_ms,
            duration_ms,
            total_damage: total,
            target,
            actors,
            live,
        }
    }
}

fn main_target(e: &Encounter) -> Option<EntityId> {
    e.damage_by_target
        .iter()
        .max_by_key(|(id, d)| (**d, std::cmp::Reverse(**id)))
        .map(|(id, _)| *id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use protocol::decode::SpecialFlags;

    const NO: SpecialFlags = SpecialFlags {
        back: false,
        parry: false,
        perfect: false,
        double: false,
        endure: false,
        restoration: false,
    };

    fn hit(actor: u32, target: u32, skill: u32, dmg: u32) -> GameEvent {
        GameEvent::Damage(DamageEvent {
            actor,
            target,
            skill_raw: skill,
            damage: dmg,
            hits: 0,
            dot: false,
            specials: NO,
        })
    }

    fn me(e: &mut Engine) {
        e.handle(
            &GameEvent::OwnCharacter {
                entity: 1,
                name: "Eu".into(),
                server: None,
                job_code: Some(26),
            },
            0,
        );
    }

    #[test]
    fn aggregates_and_computes_dps() {
        let mut e = Engine::new(EngineConfig::default());
        me(&mut e);
        e.handle(&hit(1, 100, 15_010_013, 1_000), 1_000);
        e.handle(&hit(2, 100, 11_020_010, 3_000), 2_000);
        e.handle(&hit(1, 100, 15_010_011, 1_000), 9_000);
        let s = e.snapshot().unwrap();
        assert_eq!(s.duration_ms, 8_000);
        assert_eq!(s.total_damage, 5_000);
        assert_eq!(s.actors[0].entity, 2);
        assert_eq!(s.actors[0].class, Some(Class::Gladiator));
        let mine = &s.actors[1];
        assert!(mine.is_self);
        assert_eq!(mine.name, "Eu");
        assert_eq!(mine.class, Some(Class::Sorcerer));
        assert!((mine.dps - 250.0).abs() < 1e-9);
        assert_eq!(mine.skills.len(), 1, "níveis da mesma skill agrupados");
        assert_eq!(s.target.unwrap().entity, 100);
    }

    #[test]
    fn summons_count_for_owner() {
        let mut e = Engine::new(EngineConfig::default());
        me(&mut e);
        e.handle(&GameEvent::SummonOwner { summon: 50, owner: 1 }, 0);
        e.handle(&hit(50, 100, 16_000_000, 400), 1_000);
        let s = e.snapshot().unwrap();
        assert_eq!(s.actors.len(), 1);
        assert_eq!(s.actors[0].entity, 1);
    }

    #[test]
    fn ignores_monster_damage_on_players() {
        let mut e = Engine::new(EngineConfig::default());
        me(&mut e);
        e.handle(&hit(100, 1, 90_000_000, 9_999), 1_000);
        assert!(e.snapshot().is_none());
        assert_eq!(e.ignored_damage(), 1);
    }

    #[test]
    fn idle_timeout_closes_encounter() {
        let mut e = Engine::new(EngineConfig { idle_timeout_ms: 5_000 });
        e.handle(&hit(2, 100, 11_000_000, 10), 1_000);
        assert!(e.tick(5_000).is_none());
        let closed = e.tick(7_000).unwrap();
        assert!(!closed.live);
        assert_eq!(closed.id, 1);
        e.handle(&hit(2, 101, 11_000_000, 10), 8_000);
        assert_eq!(e.snapshot().unwrap().id, 2);
    }

    #[test]
    fn main_target_leaving_combat_closes() {
        let mut e = Engine::new(EngineConfig::default());
        e.handle(&hit(2, 100, 11_000_000, 10), 1_000);
        let closed = e.handle(
            &GameEvent::BattleToggle {
                entity: 100,
                in_battle: false,
            },
            1_500,
        );
        assert!(closed.is_some());
        assert!(e.snapshot().is_none());
    }

    #[test]
    fn hit_list_does_not_multiply_damage() {
        let mut e = Engine::new(EngineConfig::default());
        let ev = GameEvent::Damage(DamageEvent {
            actor: 2,
            target: 100,
            skill_raw: 11_000_000,
            damage: 100,
            hits: 3,
            dot: false,
            specials: NO,
        });
        e.handle(&ev, 0);
        let s = e.snapshot().unwrap();
        // No SA o dano do pacote já é o total do golpe (conferido em gravações).
        assert_eq!((s.total_damage, s.actors[0].hits, s.actors[0].max_hit), (100, 1, 100));
    }
}
