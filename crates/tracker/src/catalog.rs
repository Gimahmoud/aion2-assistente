//! Dados fixos que vêm com o app (`data/tracker/*.json`).

use serde::{Deserialize, Serialize};

use crate::clock::{Period, ServerClock};
use crate::schedule::EventRule;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PetDef {
    pub id: String,
    pub nome: String,
    pub grupo: String,
    #[serde(default)]
    pub origem: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Scope {
    Personagem,
    Servidor,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TaskDef {
    pub id: String,
    pub nome: String,
    pub periodo: Period,
    pub meta: u32,
    pub escopo: Scope,
}

#[derive(Deserialize)]
struct PetsFile {
    mascotes: Vec<PetDef>,
}
#[derive(Deserialize)]
struct TasksFile {
    tarefas: Vec<TaskDef>,
}
#[derive(Deserialize)]
struct EventsFile {
    eventos: Vec<EventRule>,
}

#[derive(Debug, Clone)]
pub struct Catalog {
    pub clock: ServerClock,
    pub pets: Vec<PetDef>,
    pub tasks: Vec<TaskDef>,
    pub events: Vec<EventRule>,
}

impl Catalog {
    pub fn from_json(clock: &str, pets: &str, tasks: &str, events: &str) -> serde_json::Result<Self> {
        Ok(Self {
            clock: serde_json::from_str(clock)?,
            pets: serde_json::from_str::<PetsFile>(pets)?.mascotes,
            tasks: serde_json::from_str::<TasksFile>(tasks)?.tarefas,
            events: serde_json::from_str::<EventsFile>(events)?.eventos,
        })
    }

    /// Catálogo embutido no binário.
    pub fn bundled() -> Self {
        Self::from_json(
            include_str!("../../../data/tracker/servidor.json"),
            include_str!("../../../data/tracker/mascotes.json"),
            include_str!("../../../data/tracker/tarefas.json"),
            include_str!("../../../data/tracker/eventos.json"),
        )
        .expect("data/tracker/*.json inválido")
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn bundled_data_is_valid_and_ids_unique() {
        let c = Catalog::bundled();
        assert!(!c.pets.is_empty() && !c.tasks.is_empty() && !c.events.is_empty());
        let unique = |ids: Vec<&str>| ids.len() == ids.iter().collect::<HashSet<_>>().len();
        assert!(unique(c.pets.iter().map(|p| p.id.as_str()).collect()));
        assert!(unique(c.tasks.iter().map(|t| t.id.as_str()).collect()));
        assert!(unique(c.events.iter().map(|e| e.id.as_str()).collect()));
        assert!(c.tasks.iter().all(|t| t.meta > 0));
    }
}
