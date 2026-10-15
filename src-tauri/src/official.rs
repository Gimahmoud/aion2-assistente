//! Perfil do personagem na API oficial da NCSoft (a mesma do site
//! aion2.plaync.com e de sites como o shugo.gg). A API não é documentada:
//! guardamos a resposta como veio e a interface lê os campos que conhece.

use std::time::Duration;

use serde_json::{json, Value};
use tauri::State;
use tracker::OfficialLink;

use crate::tracker_cmds::TrackerState;

const BASE: &str = "https://aion2.plaync.com";
const SEARCH: &str = "https://api-search.plaync.com/aion2global/search/v2/character";
const LANG: &str = "en-US";

type CmdResult<T> = Result<T, String>;

fn client() -> CmdResult<reqwest::Client> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .user_agent(concat!("Aion2Assistente/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| e.to_string())
}

async fn get_json(url: &str, query: &[(&str, &str)]) -> CmdResult<Value> {
    let resp = client()?
        .get(url)
        .query(query)
        .header("accept", "application/json")
        .header("referer", "https://aion2.plaync.com/en-us/characters/")
        .send()
        .await
        .map_err(|e| format!("sem resposta do site oficial: {e}"))?;
    let status = resp.status();
    if !status.is_success() {
        return Err(format!("o site oficial respondeu {status}"));
    }
    resp.json()
        .await
        .map_err(|e| format!("resposta inesperada do site oficial: {e}"))
}

/// Busca personagens pelo nome. Devolve a lista como veio (`data.list` ou `list`).
#[tauri::command]
pub async fn official_search(region: String, name: String) -> CmdResult<Vec<Value>> {
    let name = name.trim();
    if name.is_empty() {
        return Err("digite o nome do personagem".into());
    }
    let body = get_json(
        SEARCH,
        &[
            ("keyword", name),
            ("region", &region),
            ("localeInfo", LANG),
            ("page", "1"),
            ("size", "30"),
        ],
    )
    .await?;
    let list = body
        .pointer("/data/list")
        .or_else(|| body.get("list"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    Ok(list)
}

#[tauri::command]
pub fn official_link(state: State<'_, TrackerState>, character_id: i64, link: Option<OfficialLink>) -> CmdResult<()> {
    state
        .tracker
        .lock()
        .unwrap()
        .set_official_link(character_id, link.as_ref())
        .map_err(|e| e.to_string())
}

/// Baixa info e equipamentos do personagem ligado e guarda a cópia.
#[tauri::command]
pub async fn official_refresh(state: State<'_, TrackerState>, character_id: i64) -> CmdResult<Value> {
    let link = {
        let t = state.tracker.lock().unwrap();
        t.character(character_id)
            .map_err(|e| e.to_string())?
            .official
            .ok_or("este personagem ainda não está ligado a um perfil oficial")?
    };
    let q = [
        ("region", link.region.as_str()),
        ("lang", LANG),
        ("characterId", link.character_id.as_str()),
        ("serverId", link.server_id.as_str()),
    ];
    let info = get_json(&format!("{BASE}/api/character/info"), &q).await?;
    // Equipamento é um extra: se falhar, o perfil sai sem ele.
    let equipment = get_json(&format!("{BASE}/api/character/equipment"), &q)
        .await
        .unwrap_or(Value::Null);
    let profile = json!({ "info": info, "equipment": equipment });
    let now = crate::runtime::now_ms() as i64;
    state
        .tracker
        .lock()
        .unwrap()
        .save_profile(character_id, &profile.to_string(), now)
        .map_err(|e| e.to_string())?;
    Ok(json!({ "json": profile.to_string(), "fetched_at_ms": now }))
}

#[tauri::command]
pub fn official_cached(state: State<'_, TrackerState>, character_id: i64) -> CmdResult<Option<tracker::CachedProfile>> {
    state
        .tracker
        .lock()
        .unwrap()
        .cached_profile(character_id)
        .map_err(|e| e.to_string())
}
