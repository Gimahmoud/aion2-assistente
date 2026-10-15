//! Banco local do tracker (`tracker.db`), separado do histórico de lutas.

use std::path::Path;

use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;

use crate::catalog::{PetDef, Scope, TaskDef};
use crate::clock::Period;

#[derive(Debug, thiserror::Error)]
pub enum TrackerError {
    #[error("sqlite: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("{0}")]
    Invalid(String),
}

pub type Result<T> = std::result::Result<T, TrackerError>;

const MIGRATIONS: &[&str] = &[
    r#"
CREATE TABLE characters (
    id            INTEGER PRIMARY KEY,
    name          TEXT NOT NULL,
    class         TEXT,
    created_at_ms INTEGER NOT NULL
);

-- source: 'manual' agora; 'pacote' quando o tracker automático existir.
CREATE TABLE pets_owned (
    character_id INTEGER NOT NULL REFERENCES characters(id) ON DELETE CASCADE,
    pet_id       TEXT NOT NULL,
    source       TEXT NOT NULL DEFAULT 'manual',
    marked_at_ms INTEGER NOT NULL,
    PRIMARY KEY (character_id, pet_id)
);

-- Contagem vale só enquanto period_start_ms for o período atual.
CREATE TABLE task_progress (
    character_id    INTEGER NOT NULL REFERENCES characters(id) ON DELETE CASCADE,
    task_id         TEXT NOT NULL,
    period_start_ms INTEGER NOT NULL,
    count           INTEGER NOT NULL,
    PRIMARY KEY (character_id, task_id)
);

CREATE TABLE custom_pets (
    id     INTEGER PRIMARY KEY,
    name   TEXT NOT NULL,
    grupo  TEXT NOT NULL,
    origem TEXT NOT NULL
);

CREATE TABLE custom_tasks (
    id      INTEGER PRIMARY KEY,
    name    TEXT NOT NULL,
    periodo TEXT NOT NULL,
    meta    INTEGER NOT NULL
);

CREATE TABLE alert_settings (
    event_id TEXT PRIMARY KEY,
    enabled  INTEGER NOT NULL,
    lead_min INTEGER NOT NULL
);
"#,
    r#"
-- Ligação com o perfil no site oficial e a última cópia baixada dele.
ALTER TABLE characters ADD COLUMN official_region TEXT;
ALTER TABLE characters ADD COLUMN official_server_id TEXT;
ALTER TABLE characters ADD COLUMN official_character_id TEXT;
ALTER TABLE characters ADD COLUMN profile_json TEXT;
ALTER TABLE characters ADD COLUMN profile_fetched_ms INTEGER;
"#,
];

pub const CUSTOM_PREFIX: &str = "u:";

/// Onde achar o personagem na API oficial da NCSoft.
#[derive(Debug, Clone, PartialEq, Serialize, serde::Deserialize)]
pub struct OfficialLink {
    pub region: String,
    pub server_id: String,
    pub character_id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Character {
    pub id: i64,
    pub name: String,
    pub class: Option<String>,
    pub official: Option<OfficialLink>,
}

/// Última cópia do perfil oficial, guardada como veio da API.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CachedProfile {
    pub json: String,
    pub fetched_at_ms: i64,
}

pub struct TrackerStore {
    conn: Connection,
}

fn period_str(p: Period) -> &'static str {
    match p {
        Period::Diario => "diario",
        Period::Semanal => "semanal",
    }
}

impl TrackerStore {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).ok();
        }
        Self::init(Connection::open(path)?)
    }

    pub fn open_in_memory() -> Result<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> Result<Self> {
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        let mut s = Self { conn };
        let current: i64 = s.conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
        for (i, sql) in MIGRATIONS.iter().enumerate().skip(current as usize) {
            let tx = s.conn.transaction()?;
            tx.execute_batch(sql)?;
            tx.pragma_update(None, "user_version", (i + 1) as i64)?;
            tx.commit()?;
        }
        Ok(s)
    }

    pub fn characters(&self) -> Result<Vec<Character>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, class, official_region, official_server_id, official_character_id
             FROM characters ORDER BY id",
        )?;
        let rows = stmt.query_map([], |r| {
            let link: (Option<String>, Option<String>, Option<String>) = (r.get(3)?, r.get(4)?, r.get(5)?);
            Ok(Character {
                id: r.get(0)?,
                name: r.get(1)?,
                class: r.get(2)?,
                official: match link {
                    (Some(region), Some(server_id), Some(character_id)) => Some(OfficialLink {
                        region,
                        server_id,
                        character_id,
                    }),
                    _ => None,
                },
            })
        })?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    pub fn add_character(&self, name: &str, class: Option<&str>, now_ms: i64) -> Result<Character> {
        self.conn.execute(
            "INSERT INTO characters (name, class, created_at_ms) VALUES (?1, ?2, ?3)",
            params![name, class, now_ms],
        )?;
        Ok(Character {
            id: self.conn.last_insert_rowid(),
            name: name.into(),
            class: class.map(Into::into),
            official: None,
        })
    }

    pub fn update_character(&self, id: i64, name: &str, class: Option<&str>) -> Result<()> {
        self.conn.execute(
            "UPDATE characters SET name = ?2, class = ?3 WHERE id = ?1",
            params![id, name, class],
        )?;
        Ok(())
    }

    /// Liga (ou desliga, com `None`) o personagem a um perfil oficial. Trocar
    /// a ligação apaga a cópia do perfil antigo.
    pub fn set_official_link(&self, id: i64, link: Option<&OfficialLink>) -> Result<()> {
        self.conn.execute(
            "UPDATE characters SET official_region = ?2, official_server_id = ?3, official_character_id = ?4,
                 profile_json = NULL, profile_fetched_ms = NULL
             WHERE id = ?1",
            params![
                id,
                link.map(|l| &l.region),
                link.map(|l| &l.server_id),
                link.map(|l| &l.character_id)
            ],
        )?;
        Ok(())
    }

    pub fn save_profile(&self, id: i64, json: &str, now_ms: i64) -> Result<()> {
        self.conn.execute(
            "UPDATE characters SET profile_json = ?2, profile_fetched_ms = ?3 WHERE id = ?1",
            params![id, json, now_ms],
        )?;
        Ok(())
    }

    pub fn cached_profile(&self, id: i64) -> Result<Option<CachedProfile>> {
        let row: Option<(Option<String>, Option<i64>)> = self
            .conn
            .query_row(
                "SELECT profile_json, profile_fetched_ms FROM characters WHERE id = ?1",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        Ok(match row {
            Some((Some(json), Some(fetched_at_ms))) => Some(CachedProfile { json, fetched_at_ms }),
            _ => None,
        })
    }

    pub fn remove_character(&self, id: i64) -> Result<()> {
        self.conn.execute("DELETE FROM characters WHERE id = ?1", [id])?;
        Ok(())
    }

    pub fn owned_pets(&self, character_id: i64) -> Result<Vec<String>> {
        let mut stmt = self
            .conn
            .prepare("SELECT pet_id FROM pets_owned WHERE character_id = ?1")?;
        let rows = stmt.query_map([character_id], |r| r.get(0))?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    pub fn set_pet_owned(&self, character_id: i64, pet_id: &str, owned: bool, now_ms: i64) -> Result<()> {
        if owned {
            self.conn.execute(
                "INSERT INTO pets_owned (character_id, pet_id, source, marked_at_ms) VALUES (?1, ?2, 'manual', ?3)
                 ON CONFLICT(character_id, pet_id) DO NOTHING",
                params![character_id, pet_id, now_ms],
            )?;
        } else {
            self.conn.execute(
                "DELETE FROM pets_owned WHERE character_id = ?1 AND pet_id = ?2",
                params![character_id, pet_id],
            )?;
        }
        Ok(())
    }

    /// Contagem de cada tarefa no período cujo início é dado por `period_start`.
    pub fn task_count(&self, character_id: i64, task_id: &str, period_start_ms: i64) -> Result<u32> {
        let row: Option<(i64, u32)> = self
            .conn
            .query_row(
                "SELECT period_start_ms, count FROM task_progress WHERE character_id = ?1 AND task_id = ?2",
                params![character_id, task_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        Ok(match row {
            Some((p, c)) if p == period_start_ms => c,
            _ => 0,
        })
    }

    pub fn set_task_count(&self, character_id: i64, task_id: &str, period_start_ms: i64, count: u32) -> Result<()> {
        self.conn.execute(
            "INSERT INTO task_progress (character_id, task_id, period_start_ms, count) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(character_id, task_id) DO UPDATE
             SET period_start_ms = excluded.period_start_ms, count = excluded.count",
            params![character_id, task_id, period_start_ms, count],
        )?;
        Ok(())
    }

    pub fn custom_pets(&self) -> Result<Vec<PetDef>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, name, grupo, origem FROM custom_pets ORDER BY id")?;
        let rows = stmt.query_map([], |r| {
            Ok(PetDef {
                id: format!("{CUSTOM_PREFIX}{}", r.get::<_, i64>(0)?),
                nome: r.get(1)?,
                grupo: r.get(2)?,
                origem: r.get(3)?,
            })
        })?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    pub fn add_custom_pet(&self, nome: &str, grupo: &str, origem: &str) -> Result<String> {
        self.conn.execute(
            "INSERT INTO custom_pets (name, grupo, origem) VALUES (?1, ?2, ?3)",
            params![nome, grupo, origem],
        )?;
        Ok(format!("{CUSTOM_PREFIX}{}", self.conn.last_insert_rowid()))
    }

    pub fn custom_tasks(&self) -> Result<Vec<TaskDef>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, name, periodo, meta FROM custom_tasks ORDER BY id")?;
        let rows = stmt.query_map([], |r| {
            let periodo: String = r.get(2)?;
            Ok(TaskDef {
                id: format!("{CUSTOM_PREFIX}{}", r.get::<_, i64>(0)?),
                nome: r.get(1)?,
                periodo: if periodo == "semanal" {
                    Period::Semanal
                } else {
                    Period::Diario
                },
                meta: r.get(3)?,
                escopo: Scope::Personagem,
            })
        })?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    pub fn add_custom_task(&self, nome: &str, periodo: Period, meta: u32) -> Result<String> {
        self.conn.execute(
            "INSERT INTO custom_tasks (name, periodo, meta) VALUES (?1, ?2, ?3)",
            params![nome, period_str(periodo), meta],
        )?;
        Ok(format!("{CUSTOM_PREFIX}{}", self.conn.last_insert_rowid()))
    }

    /// Remove um mascote criado pelo usuário, com as marcações dele.
    pub fn remove_custom_pet(&mut self, id: &str) -> Result<()> {
        let n: i64 = id
            .strip_prefix(CUSTOM_PREFIX)
            .and_then(|n| n.parse().ok())
            .ok_or_else(|| TrackerError::Invalid(format!("não é um item personalizado: {id}")))?;
        let tx = self.conn.transaction()?;
        tx.execute("DELETE FROM custom_pets WHERE id = ?1", [n])?;
        tx.execute("DELETE FROM pets_owned WHERE pet_id = ?1", [id])?;
        tx.commit()?;
        Ok(())
    }

    pub fn remove_custom_task(&mut self, id: &str) -> Result<()> {
        let n: i64 = id
            .strip_prefix(CUSTOM_PREFIX)
            .and_then(|n| n.parse().ok())
            .ok_or_else(|| TrackerError::Invalid(format!("não é um item personalizado: {id}")))?;
        let tx = self.conn.transaction()?;
        tx.execute("DELETE FROM custom_tasks WHERE id = ?1", [n])?;
        tx.execute("DELETE FROM task_progress WHERE task_id = ?1", [id])?;
        tx.commit()?;
        Ok(())
    }

    /// `(ligado, antecedência)` salvo para um evento, se o usuário já mexeu nele.
    pub fn alert_setting(&self, event_id: &str) -> Result<Option<(bool, u32)>> {
        Ok(self
            .conn
            .query_row(
                "SELECT enabled, lead_min FROM alert_settings WHERE event_id = ?1",
                [event_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?)
    }

    pub fn set_alert_setting(&self, event_id: &str, enabled: bool, lead_min: u32) -> Result<()> {
        self.conn.execute(
            "INSERT INTO alert_settings (event_id, enabled, lead_min) VALUES (?1, ?2, ?3)
             ON CONFLICT(event_id) DO UPDATE SET enabled = excluded.enabled, lead_min = excluded.lead_min",
            params![event_id, enabled, lead_min],
        )?;
        Ok(())
    }
}
