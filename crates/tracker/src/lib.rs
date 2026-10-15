//! Tracker de personagem e agenda de eventos.
//!
//! Primeira versão manual: o jogador marca os mascotes que tem e conta as
//! diárias e semanais; a contagem zera sozinha no reset do servidor. Os alertas
//! saem de uma agenda fixa em `data/tracker/eventos.json`. Nada aqui lê o jogo.

pub mod catalog;
pub mod clock;
pub mod schedule;
pub mod store;

use std::collections::HashSet;
use std::path::Path;

use serde::Serialize;

pub use catalog::{Catalog, PetDef, Scope, TaskDef};
pub use clock::{Period, ServerClock};
pub use schedule::{EventRule, Occurrence};
pub use store::{CachedProfile, Character, OfficialLink, Result, TrackerError, TrackerStore};

use clock::MINUTE_MS;

#[derive(Debug, Clone, Serialize)]
pub struct PetStatus {
    #[serde(flatten)]
    pub def: PetDef,
    pub custom: bool,
    pub owned: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct TaskStatus {
    #[serde(flatten)]
    pub def: TaskDef,
    pub custom: bool,
    pub count: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct Checklist {
    pub next_daily_reset_ms: i64,
    pub next_weekly_reset_ms: i64,
    pub tasks: Vec<TaskStatus>,
}

#[derive(Debug, Clone, Serialize)]
pub struct EventConfig {
    pub id: String,
    pub nome: String,
    pub descricao: Option<String>,
    pub enabled: bool,
    pub lead_min: u32,
    pub next_start_ms: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct UpcomingEvent {
    pub event_id: String,
    pub nome: String,
    pub start_ms: i64,
    pub end_ms: Option<i64>,
    pub enabled: bool,
}

/// Um alerta a disparar: `fire_ms` = início do evento menos a antecedência.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Alert {
    pub event_id: String,
    pub nome: String,
    pub descricao: Option<String>,
    pub start_ms: i64,
    pub fire_ms: i64,
}

pub struct Tracker {
    pub catalog: Catalog,
    store: TrackerStore,
}

const MAX_LEAD_MIN: u32 = 24 * 60;

fn invalid(msg: impl Into<String>) -> TrackerError {
    TrackerError::Invalid(msg.into())
}

impl Tracker {
    pub fn new(catalog: Catalog, store: TrackerStore) -> Self {
        Self { catalog, store }
    }

    pub fn open(path: &Path) -> Result<Self> {
        Ok(Self::new(Catalog::bundled(), TrackerStore::open(path)?))
    }

    // ---- personagens ----

    pub fn characters(&self) -> Result<Vec<Character>> {
        self.store.characters()
    }

    pub fn add_character(&self, name: &str, class: Option<&str>, now_ms: i64) -> Result<Character> {
        let name = name.trim();
        if name.is_empty() {
            return Err(invalid("o nome do personagem está vazio"));
        }
        self.store.add_character(name, class, now_ms)
    }

    pub fn update_character(&self, id: i64, name: &str, class: Option<&str>) -> Result<()> {
        let name = name.trim();
        if name.is_empty() {
            return Err(invalid("o nome do personagem está vazio"));
        }
        self.store.update_character(id, name, class)
    }

    pub fn remove_character(&self, id: i64) -> Result<()> {
        self.store.remove_character(id)
    }

    pub fn character(&self, id: i64) -> Result<Character> {
        self.store
            .characters()?
            .into_iter()
            .find(|c| c.id == id)
            .ok_or_else(|| invalid(format!("personagem desconhecido: {id}")))
    }

    // ---- perfil oficial ----

    pub fn set_official_link(&self, id: i64, link: Option<&OfficialLink>) -> Result<()> {
        self.character(id)?;
        self.store.set_official_link(id, link)
    }

    pub fn save_profile(&self, id: i64, json: &str, now_ms: i64) -> Result<()> {
        self.store.save_profile(id, json, now_ms)
    }

    pub fn cached_profile(&self, id: i64) -> Result<Option<CachedProfile>> {
        self.store.cached_profile(id)
    }

    // ---- mascotes ----

    fn all_pets(&self) -> Result<Vec<(PetDef, bool)>> {
        let mut out: Vec<_> = self.catalog.pets.iter().cloned().map(|p| (p, false)).collect();
        out.extend(self.store.custom_pets()?.into_iter().map(|p| (p, true)));
        Ok(out)
    }

    pub fn pets(&self, character_id: i64) -> Result<Vec<PetStatus>> {
        let owned: HashSet<String> = self.store.owned_pets(character_id)?.into_iter().collect();
        Ok(self
            .all_pets()?
            .into_iter()
            .map(|(def, custom)| PetStatus {
                owned: owned.contains(&def.id),
                def,
                custom,
            })
            .collect())
    }

    pub fn set_pet_owned(&self, character_id: i64, pet_id: &str, owned: bool, now_ms: i64) -> Result<()> {
        if !self.all_pets()?.iter().any(|(p, _)| p.id == pet_id) {
            return Err(invalid(format!("mascote desconhecido: {pet_id}")));
        }
        self.store.set_pet_owned(character_id, pet_id, owned, now_ms)
    }

    pub fn add_custom_pet(&self, nome: &str, grupo: &str, origem: &str) -> Result<String> {
        if nome.trim().is_empty() {
            return Err(invalid("o nome do mascote está vazio"));
        }
        let grupo = if grupo.trim().is_empty() {
            "Outros"
        } else {
            grupo.trim()
        };
        self.store.add_custom_pet(nome.trim(), grupo, origem.trim())
    }

    pub fn remove_custom_pet(&mut self, id: &str) -> Result<()> {
        self.store.remove_custom_pet(id)
    }

    // ---- diárias e semanais ----

    fn all_tasks(&self) -> Result<Vec<(TaskDef, bool)>> {
        let mut out: Vec<_> = self.catalog.tasks.iter().cloned().map(|t| (t, false)).collect();
        out.extend(self.store.custom_tasks()?.into_iter().map(|t| (t, true)));
        Ok(out)
    }

    pub fn checklist(&self, character_id: i64, now_ms: i64) -> Result<Checklist> {
        let clock = &self.catalog.clock;
        let tasks = self
            .all_tasks()?
            .into_iter()
            .map(|(def, custom)| {
                let start = clock.period_start(def.periodo, now_ms);
                let count = self.store.task_count(character_id, &def.id, start)?.min(def.meta);
                Ok(TaskStatus { def, custom, count })
            })
            .collect::<Result<_>>()?;
        Ok(Checklist {
            next_daily_reset_ms: clock.next_reset(Period::Diario, now_ms),
            next_weekly_reset_ms: clock.next_reset(Period::Semanal, now_ms),
            tasks,
        })
    }

    pub fn set_task_count(&self, character_id: i64, task_id: &str, count: u32, now_ms: i64) -> Result<()> {
        let (def, _) = self
            .all_tasks()?
            .into_iter()
            .find(|(t, _)| t.id == task_id)
            .ok_or_else(|| invalid(format!("tarefa desconhecida: {task_id}")))?;
        let start = self.catalog.clock.period_start(def.periodo, now_ms);
        self.store
            .set_task_count(character_id, task_id, start, count.min(def.meta))
    }

    pub fn add_custom_task(&self, nome: &str, periodo: Period, meta: u32) -> Result<String> {
        if nome.trim().is_empty() {
            return Err(invalid("o nome da tarefa está vazio"));
        }
        self.store.add_custom_task(nome.trim(), periodo, meta.clamp(1, 999))
    }

    pub fn remove_custom_task(&mut self, id: &str) -> Result<()> {
        self.store.remove_custom_task(id)
    }

    // ---- eventos e alertas ----

    fn alert_of(&self, rule: &EventRule) -> Result<(bool, u32)> {
        Ok(self
            .store
            .alert_setting(&rule.id)?
            .unwrap_or((rule.alerta_padrao, rule.aviso_min)))
    }

    pub fn events(&self, now_ms: i64) -> Result<Vec<EventConfig>> {
        self.catalog
            .events
            .iter()
            .map(|r| {
                let (enabled, lead_min) = self.alert_of(r)?;
                Ok(EventConfig {
                    id: r.id.clone(),
                    nome: r.nome.clone(),
                    descricao: r.descricao.clone(),
                    enabled,
                    lead_min,
                    next_start_ms: r.next_start(&self.catalog.clock, now_ms),
                })
            })
            .collect()
    }

    pub fn set_alert(&self, event_id: &str, enabled: bool, lead_min: u32) -> Result<()> {
        if !self.catalog.events.iter().any(|e| e.id == event_id) {
            return Err(invalid(format!("evento desconhecido: {event_id}")));
        }
        self.store
            .set_alert_setting(event_id, enabled, lead_min.min(MAX_LEAD_MIN))
    }

    /// Ocorrências em andamento ou nas próximas `hours` horas, em ordem.
    pub fn upcoming(&self, now_ms: i64, hours: u32) -> Result<Vec<UpcomingEvent>> {
        let to = now_ms + hours as i64 * 60 * MINUTE_MS;
        let mut out = Vec::new();
        for r in &self.catalog.events {
            let (enabled, _) = self.alert_of(r)?;
            for o in r.occurrences(&self.catalog.clock, now_ms, to) {
                out.push(UpcomingEvent {
                    event_id: r.id.clone(),
                    nome: r.nome.clone(),
                    start_ms: o.start_ms,
                    end_ms: o.end_ms,
                    enabled,
                });
            }
        }
        out.sort_by(|a, b| a.start_ms.cmp(&b.start_ms).then_with(|| a.nome.cmp(&b.nome)));
        Ok(out)
    }

    /// Alertas ligados com `fire_ms` em `(after_ms, until_ms]`, em ordem.
    pub fn due_alerts(&self, after_ms: i64, until_ms: i64) -> Result<Vec<Alert>> {
        let mut out = Vec::new();
        for r in &self.catalog.events {
            let (enabled, lead) = self.alert_of(r)?;
            if !enabled {
                continue;
            }
            let lead_ms = lead as i64 * MINUTE_MS;
            for start in r.starts_between(&self.catalog.clock, after_ms + lead_ms + 1, until_ms + lead_ms + 1) {
                out.push(Alert {
                    event_id: r.id.clone(),
                    nome: r.nome.clone(),
                    descricao: r.descricao.clone(),
                    start_ms: start,
                    fire_ms: start - lead_ms,
                });
            }
        }
        out.sort_by_key(|a| a.fire_ms);
        Ok(out)
    }

    /// Próximo instante em que algum alerta ligado dispara depois de `after_ms`.
    pub fn next_alert_ms(&self, after_ms: i64) -> Result<Option<i64>> {
        let mut next: Option<i64> = None;
        for r in &self.catalog.events {
            let (enabled, lead) = self.alert_of(r)?;
            if !enabled {
                continue;
            }
            let lead_ms = lead as i64 * MINUTE_MS;
            if let Some(start) = r.next_start(&self.catalog.clock, after_ms + lead_ms + 1) {
                let fire = start - lead_ms;
                next = Some(next.map_or(fire, |n| n.min(fire)));
            }
        }
        Ok(next)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clock::DAY_MS;

    // 2026-10-14 (quarta) 07:00 UTC = reset diário e semanal.
    const RESET: i64 = 1_791_961_200_000;
    const H: i64 = 60 * MINUTE_MS;

    fn tracker() -> Tracker {
        Tracker::new(Catalog::bundled(), TrackerStore::open_in_memory().unwrap())
    }

    #[test]
    fn task_counts_reset_with_server_period() {
        let t = tracker();
        let c = t.add_character("Eu", Some("sorcerer"), 0).unwrap();
        t.set_task_count(c.id, "pesadelo", 1, RESET - H).unwrap();
        t.set_task_count(c.id, "raide", 9, RESET - H).unwrap();
        let count = |now, id: &str| {
            t.checklist(c.id, now)
                .unwrap()
                .tasks
                .into_iter()
                .find(|x| x.def.id == id)
                .unwrap()
                .count
        };
        assert_eq!(count(RESET - 1, "pesadelo"), 1);
        assert_eq!(count(RESET - 1, "raide"), 4, "limitado à meta");
        // Depois do reset de quarta, diárias e semanais zeram.
        assert_eq!(count(RESET, "pesadelo"), 0);
        assert_eq!(count(RESET, "raide"), 0);
        // Diária marcada na quinta zera na sexta, semanal não.
        t.set_task_count(c.id, "pesadelo", 2, RESET + H).unwrap();
        t.set_task_count(c.id, "raide", 2, RESET + H).unwrap();
        assert_eq!(count(RESET + DAY_MS, "pesadelo"), 0);
        assert_eq!(count(RESET + DAY_MS, "raide"), 2);
        let cl = t.checklist(c.id, RESET + H).unwrap();
        assert_eq!(cl.next_daily_reset_ms, RESET + DAY_MS);
        assert_eq!(cl.next_weekly_reset_ms, RESET + 7 * DAY_MS);
    }

    #[test]
    fn pets_per_character_and_custom() {
        let mut t = tracker();
        let a = t.add_character("A", None, 0).unwrap();
        let b = t.add_character("B", None, 0).unwrap();
        t.set_pet_owned(a.id, "papis", true, 1).unwrap();
        t.set_pet_owned(a.id, "papis", true, 2).unwrap();
        assert!(t.set_pet_owned(a.id, "nao-existe", true, 1).is_err());
        let owned = |t: &Tracker, c: i64| t.pets(c).unwrap().into_iter().filter(|p| p.owned).count();
        assert_eq!(owned(&t, a.id), 1);
        assert_eq!(owned(&t, b.id), 0);

        let id = t.add_custom_pet("Novo", "", "Loja").unwrap();
        t.set_pet_owned(b.id, &id, true, 1).unwrap();
        let p = t.pets(b.id).unwrap().into_iter().find(|p| p.def.id == id).unwrap();
        assert!(p.custom && p.owned && p.def.grupo == "Outros");
        t.remove_custom_pet(&id).unwrap();
        assert_eq!(owned(&t, b.id), 0);
        assert!(t.remove_custom_pet("papis").is_err());

        t.set_pet_owned(a.id, "papis", false, 3).unwrap();
        assert_eq!(owned(&t, a.id), 0);
        t.remove_character(a.id).unwrap();
        assert_eq!(t.characters().unwrap().len(), 1);
    }

    #[test]
    fn official_link_and_profile_cache() {
        let t = tracker();
        let c = t.add_character("Eu", None, 0).unwrap();
        assert_eq!(t.character(c.id).unwrap().official, None);
        let link = OfficialLink {
            region: "la".into(),
            server_id: "2102".into(),
            character_id: "abc%3D".into(),
        };
        t.set_official_link(c.id, Some(&link)).unwrap();
        assert_eq!(t.character(c.id).unwrap().official, Some(link.clone()));
        assert_eq!(t.cached_profile(c.id).unwrap(), None);
        t.save_profile(c.id, "{\"a\":1}", 5).unwrap();
        assert_eq!(t.cached_profile(c.id).unwrap().unwrap().fetched_at_ms, 5);
        // Trocar a ligação descarta a cópia antiga.
        t.set_official_link(c.id, Some(&link)).unwrap();
        assert_eq!(t.cached_profile(c.id).unwrap(), None);
        t.set_official_link(c.id, None).unwrap();
        assert_eq!(t.character(c.id).unwrap().official, None);
        assert!(t.set_official_link(999, None).is_err());
    }

    #[test]
    fn custom_tasks() {
        let mut t = tracker();
        let c = t.add_character("Eu", None, 0).unwrap();
        let id = t.add_custom_task("Lugar secreto", Period::Semanal, 2).unwrap();
        t.set_task_count(c.id, &id, 2, RESET).unwrap();
        let task = |t: &Tracker| {
            t.checklist(c.id, RESET + H)
                .unwrap()
                .tasks
                .into_iter()
                .find(|x| x.def.id == id)
        };
        assert_eq!(task(&t).unwrap().count, 2);
        t.remove_custom_task(&id).unwrap();
        assert!(task(&t).is_none());
    }

    #[test]
    fn alerts_respect_settings_and_lead() {
        let t = tracker();
        // Só a fenda, com 10 min de antecedência.
        for e in &t.catalog.events {
            t.set_alert(&e.id, e.id == "fenda-espaco-tempo", 10).unwrap();
        }
        // Fenda abre em múltiplos de 3 h UTC; RESET (07:00 UTC) -> próxima às 09:00.
        let next = t.next_alert_ms(RESET).unwrap().unwrap();
        assert_eq!(next, RESET + 2 * H - 10 * MINUTE_MS);
        let due = t.due_alerts(RESET, next).unwrap();
        assert_eq!(due.len(), 1);
        assert_eq!(due[0].start_ms, RESET + 2 * H);
        // A janela é aberta à esquerda: o mesmo alerta não sai duas vezes.
        assert!(t.due_alerts(next, next + MINUTE_MS).unwrap().is_empty());

        t.set_alert("fenda-espaco-tempo", false, 10).unwrap();
        assert_eq!(t.next_alert_ms(RESET).unwrap(), None);
        assert!(t.set_alert("nao-existe", true, 5).is_err());
    }

    #[test]
    fn default_alert_settings_come_from_json() {
        let t = tracker();
        let ev = t.events(RESET + 1).unwrap();
        let beritra = ev.iter().find(|e| e.id == "beritra").unwrap();
        assert!(!beritra.enabled);
        let reset = ev.iter().find(|e| e.id == "reset-semanal").unwrap();
        assert!(reset.enabled && reset.lead_min == 120);
        assert_eq!(reset.next_start_ms, Some(RESET + 7 * DAY_MS));
        let up = t.upcoming(RESET, 3).unwrap();
        assert!(up.windows(2).all(|w| w[0].start_ms <= w[1].start_ms));
        assert!(up.iter().any(|u| u.event_id == "fenda-espaco-tempo"));
    }
}
