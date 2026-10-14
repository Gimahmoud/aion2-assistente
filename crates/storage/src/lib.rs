//! Banco local em SQLite (`%APPDATA%\aion2-assistente\data.db`).
//!
//! O histórico guarda um resumo por luta, uma linha por jogador (para
//! consultas como "meu DPS ao longo do tempo") e o detalhe completo em JSON.

use std::path::Path;

use combat::EncounterSnapshot;
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::Value;

#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("sqlite: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, StorageError>;

/// Migrações em ordem. A posição + 1 é gravada em `PRAGMA user_version`.
const MIGRATIONS: &[&str] = &[r#"
CREATE TABLE encounters (
    id              INTEGER PRIMARY KEY,
    started_at_ms   INTEGER NOT NULL,
    duration_ms     INTEGER NOT NULL,
    total_damage    INTEGER NOT NULL,
    target_entity   INTEGER,
    target_mob_code INTEGER,
    self_dps        REAL,
    parser_version  TEXT NOT NULL,
    detail_json     TEXT NOT NULL
);
CREATE INDEX encounters_started ON encounters(started_at_ms);

CREATE TABLE encounter_actors (
    encounter_id INTEGER NOT NULL REFERENCES encounters(id) ON DELETE CASCADE,
    entity       INTEGER NOT NULL,
    name         TEXT NOT NULL,
    class        TEXT,
    is_self      INTEGER NOT NULL,
    damage       INTEGER NOT NULL,
    dps          REAL NOT NULL,
    share        REAL NOT NULL,
    PRIMARY KEY (encounter_id, entity)
);
CREATE INDEX encounter_actors_name ON encounter_actors(name);

CREATE TABLE settings (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
"#];

/// Linha resumida para a lista de histórico.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct EncounterRow {
    pub id: i64,
    pub started_at_ms: i64,
    pub duration_ms: i64,
    pub total_damage: i64,
    pub target_mob_code: Option<i64>,
    pub self_dps: Option<f64>,
}

pub struct Store {
    conn: Connection,
}

impl Store {
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
        let mut store = Self { conn };
        store.migrate()?;
        Ok(store)
    }

    fn migrate(&mut self) -> Result<()> {
        let current = self.schema_version()?;
        for (i, sql) in MIGRATIONS.iter().enumerate().skip(current) {
            let tx = self.conn.transaction()?;
            tx.execute_batch(sql)?;
            tx.pragma_update(None, "user_version", (i + 1) as i64)?;
            tx.commit()?;
        }
        Ok(())
    }

    pub fn schema_version(&self) -> Result<usize> {
        let v: i64 = self.conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
        Ok(v as usize)
    }

    pub fn save_encounter(&mut self, e: &EncounterSnapshot, parser_version: &str) -> Result<i64> {
        let self_dps = e.actors.iter().find(|a| a.is_self).map(|a| a.dps);
        let tx = self.conn.transaction()?;
        tx.execute(
            "INSERT INTO encounters (started_at_ms, duration_ms, total_damage, target_entity,
                 target_mob_code, self_dps, parser_version, detail_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                e.started_at_ms as i64,
                e.duration_ms as i64,
                e.total_damage as i64,
                e.target.as_ref().map(|t| t.entity),
                e.target.as_ref().and_then(|t| t.mob_code),
                self_dps,
                parser_version,
                serde_json::to_string(e)?,
            ],
        )?;
        let id = tx.last_insert_rowid();
        {
            let mut stmt = tx.prepare(
                "INSERT INTO encounter_actors (encounter_id, entity, name, class, is_self, damage, dps, share)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            )?;
            for a in &e.actors {
                let class = a.class.map(serde_json::to_value).transpose()?.and_then(|v| match v {
                    Value::String(s) => Some(s),
                    _ => None,
                });
                stmt.execute(params![
                    id,
                    a.entity,
                    a.name,
                    class,
                    a.is_self,
                    a.damage as i64,
                    a.dps,
                    a.share
                ])?;
            }
        }
        tx.commit()?;
        Ok(id)
    }

    pub fn list_encounters(&self, limit: u32) -> Result<Vec<EncounterRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, started_at_ms, duration_ms, total_damage, target_mob_code, self_dps
             FROM encounters ORDER BY started_at_ms DESC, id DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map([limit], |r| {
            Ok(EncounterRow {
                id: r.get(0)?,
                started_at_ms: r.get(1)?,
                duration_ms: r.get(2)?,
                total_damage: r.get(3)?,
                target_mob_code: r.get(4)?,
                self_dps: r.get(5)?,
            })
        })?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    pub fn get_encounter(&self, id: i64) -> Result<Option<EncounterSnapshot>> {
        let json: Option<String> = self
            .conn
            .query_row("SELECT detail_json FROM encounters WHERE id = ?1", [id], |r| r.get(0))
            .optional()?;
        Ok(json.map(|j| serde_json::from_str(&j)).transpose()?)
    }

    /// Apaga lutas mais antigas que `before_ms`. Devolve quantas foram apagadas.
    pub fn prune_before(&self, before_ms: u64) -> Result<usize> {
        Ok(self
            .conn
            .execute("DELETE FROM encounters WHERE started_at_ms < ?1", [before_ms as i64])?)
    }

    pub fn get_setting(&self, key: &str) -> Result<Option<String>> {
        Ok(self
            .conn
            .query_row("SELECT value FROM settings WHERE key = ?1", [key], |r| r.get(0))
            .optional()?)
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            [key, value],
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use combat::{ActorStats, Class, EncounterSnapshot, SkillStats, TargetInfo};

    fn sample(start: u64) -> EncounterSnapshot {
        EncounterSnapshot {
            id: 1,
            started_at_ms: start,
            duration_ms: 30_000,
            total_damage: 90_000,
            target: Some(TargetInfo {
                entity: 500,
                mob_code: Some(2_100_123),
                hp: None,
            }),
            actors: vec![ActorStats {
                entity: 1,
                name: "Eu".into(),
                class: Some(Class::Sorcerer),
                is_self: true,
                damage: 90_000,
                dps: 3_000.0,
                share: 1.0,
                hits: 30,
                max_hit: 9_000,
                perfect_hits: 2,
                back_hits: 0,
                skills: vec![SkillStats {
                    skill: 15_010_010,
                    damage: 90_000,
                    hits: 30,
                    max_hit: 9_000,
                    dot: false,
                }],
            }],
            live: false,
        }
    }

    #[test]
    fn save_list_get_prune() {
        let mut s = Store::open_in_memory().unwrap();
        assert_eq!(s.schema_version().unwrap(), MIGRATIONS.len());
        let a = s.save_encounter(&sample(1_000), "test").unwrap();
        s.save_encounter(&sample(5_000), "test").unwrap();
        let list = s.list_encounters(10).unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].started_at_ms, 5_000);
        assert_eq!(list[0].self_dps, Some(3_000.0));
        assert_eq!(s.get_encounter(a).unwrap().unwrap(), sample(1_000));
        assert_eq!(s.prune_before(2_000).unwrap(), 1);
        assert_eq!(s.list_encounters(10).unwrap().len(), 1);
        let n: i64 = s
            .conn
            .query_row("SELECT COUNT(*) FROM encounter_actors", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 1, "cascade apagou os atores da luta removida");
    }

    #[test]
    fn settings_upsert() {
        let s = Store::open_in_memory().unwrap();
        assert_eq!(s.get_setting("x").unwrap(), None);
        s.set_setting("x", "1").unwrap();
        s.set_setting("x", "2").unwrap();
        assert_eq!(s.get_setting("x").unwrap().as_deref(), Some("2"));
    }

    #[test]
    fn reopen_keeps_data() {
        let dir = std::env::temp_dir().join(format!("aion2-store-{}", std::process::id()));
        let path = dir.join("data.db");
        Store::open(&path).unwrap().save_encounter(&sample(1), "t").unwrap();
        assert_eq!(Store::open(&path).unwrap().list_encounters(5).unwrap().len(), 1);
        std::fs::remove_dir_all(dir).ok();
    }
}
