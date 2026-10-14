//! Comandos chamados pela interface (`invoke`).

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use combat::EncounterSnapshot;
use storage::{EncounterRow, Store};
use tauri::{AppHandle, Manager, State};

use crate::runtime::{Command, Runtime, Status};

pub struct AppState {
    pub runtime: Runtime,
    pub store: Arc<Mutex<Store>>,
}

type CmdResult<T> = Result<T, String>;

fn send(state: &AppState, cmd: Command) -> CmdResult<()> {
    state
        .runtime
        .cmd_tx
        .send(cmd)
        .map_err(|_| "o medidor parou".to_string())
}

#[tauri::command]
pub fn get_status(state: State<'_, AppState>) -> Status {
    state.runtime.status.lock().unwrap().clone()
}

#[tauri::command]
pub fn reset_encounter(state: State<'_, AppState>) -> CmdResult<()> {
    send(&state, Command::Reset)
}

#[tauri::command]
pub fn list_encounters(state: State<'_, AppState>, limit: Option<u32>) -> CmdResult<Vec<EncounterRow>> {
    state
        .store
        .lock()
        .unwrap()
        .list_encounters(limit.unwrap_or(100))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_encounter(state: State<'_, AppState>, id: i64) -> CmdResult<Option<EncounterSnapshot>> {
    state.store.lock().unwrap().get_encounter(id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn replay_file(state: State<'_, AppState>, path: String) -> CmdResult<()> {
    send(&state, Command::Replay(PathBuf::from(path)))
}

/// Liga ou desliga a gravação do tráfego em `%APPDATA%\...\gravacoes`.
#[tauri::command]
pub fn set_recording(app: AppHandle, state: State<'_, AppState>, enabled: bool) -> CmdResult<Option<String>> {
    let path = if enabled {
        let dir = app.path().app_data_dir().map_err(|e| e.to_string())?.join("gravacoes");
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        Some(dir.join(format!("aion2-{}.pcap", crate::runtime::now_ms())))
    } else {
        None
    };
    send(&state, Command::SetRecording(path.clone()))?;
    Ok(path.map(|p| p.display().to_string()))
}

/// Overlay "atravessável": cliques passam para o jogo.
#[tauri::command]
pub fn set_overlay_click_through(app: AppHandle, enabled: bool) -> CmdResult<()> {
    crate::set_click_through(&app, enabled).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn toggle_overlay(app: AppHandle) -> CmdResult<bool> {
    let w = app.get_webview_window("overlay").ok_or("overlay não existe")?;
    let visible = w.is_visible().map_err(|e| e.to_string())?;
    if visible { w.hide() } else { w.show() }.map_err(|e| e.to_string())?;
    Ok(!visible)
}
