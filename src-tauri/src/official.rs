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
        .timeout(Duration::from_secs(10))
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

/// Regiões da API global. Só "nae" está confirmada por outros sites; as
/// outras são tentativas, então a busca sem região tenta todas.
const REGIONS: [&str; 5] = ["la", "nae", "naw", "eu", "as"];

async fn search_region(region: String, name: String) -> CmdResult<Vec<Value>> {
    let body = get_json(
        SEARCH,
        &[
            ("keyword", name.as_str()),
            ("region", region.as_str()),
            ("localeInfo", LANG),
            ("page", "1"),
            ("size", "30"),
        ],
    )
    .await?;
    let mut list = body
        .pointer("/data/list")
        .or_else(|| body.get("list"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    for hit in &mut list {
        if let Some(o) = hit.as_object_mut() {
            o.entry("region").or_insert_with(|| Value::String(region.clone()));
        }
    }
    Ok(list)
}

/// Busca personagens pelo nome. Sem região (`None` ou vazia), procura em
/// todas ao mesmo tempo e junta os resultados.
#[tauri::command]
pub async fn official_search(region: Option<String>, name: String) -> CmdResult<Vec<Value>> {
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err("digite o nome do personagem".into());
    }
    let regions: Vec<String> = match region.filter(|r| !r.trim().is_empty()) {
        Some(r) => vec![r],
        None => REGIONS.iter().map(|r| r.to_string()).collect(),
    };
    let tasks: Vec<_> = regions
        .into_iter()
        .map(|r| tauri::async_runtime::spawn(search_region(r, name.clone())))
        .collect();
    let mut out: Vec<Value> = Vec::new();
    let mut first_err = None;
    let mut any_ok = false;
    for t in tasks {
        match t.await.map_err(|e| e.to_string()).and_then(|r| r) {
            Ok(list) => {
                any_ok = true;
                for hit in list {
                    let key = |v: &Value| (v.get("characterId").cloned(), v.get("serverId").cloned());
                    if !out.iter().any(|o| key(o) == key(&hit) && key(&hit).0.is_some()) {
                        out.push(hit);
                    }
                }
            }
            Err(e) => {
                first_err.get_or_insert(e);
            }
        }
    }
    match (any_ok, first_err) {
        (false, Some(e)) => Err(e),
        _ => Ok(out),
    }
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
    let pet = equipped_pet(&equipment);
    let profile = json!({ "info": info, "equipment": equipment });
    let now = crate::runtime::now_ms() as i64;
    let t = state.tracker.lock().unwrap();
    t.save_profile(character_id, &profile.to_string(), now)
        .map_err(|e| e.to_string())?;
    // O site oficial só mostra o mascote equipado; ele já conta como obtido.
    if let Some(pet) = pet {
        let _ = t.mark_pet_owned_by_name(character_id, &pet, "Visto no perfil oficial", now);
    }
    drop(t);
    Ok(json!({ "json": profile.to_string(), "fetched_at_ms": now }))
}

/// Nome do mascote equipado (`data.petwing.pet.name`).
fn equipped_pet(equipment: &Value) -> Option<String> {
    let data = equipment.get("data").unwrap_or(equipment);
    let pw = data.get("petwing").or_else(|| data.get("petWing"))?;
    let name = pw.pointer("/pet/name")?.as_str()?.trim();
    (!name.is_empty()).then(|| name.to_string())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_equipped_pet() {
        let eq = json!({ "data": { "petwing": { "pet": { "name": " Papis " }, "wing": { "name": "Asa" } } } });
        assert_eq!(equipped_pet(&eq).as_deref(), Some("Papis"));
        assert_eq!(equipped_pet(&json!({ "petwing": { "pet": null } })), None);
        assert_eq!(equipped_pet(&Value::Null), None);
    }
}
