//! Tracker de personagem e alertas de eventos: comandos e thread de alertas.

use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tauri::{AppHandle, Emitter, State};
use tauri_plugin_notification::NotificationExt;
use tracker::{Alert, Character, Checklist, EventConfig, Period, PetStatus, Tracker, UpcomingEvent};

pub struct TrackerState {
    pub tracker: Arc<Mutex<Tracker>>,
    /// Acorda a thread de alertas quando o usuário muda uma configuração.
    pub wake: Sender<()>,
}

type CmdResult<T> = Result<T, String>;

fn now() -> i64 {
    crate::runtime::now_ms() as i64
}

fn with<T>(state: &TrackerState, f: impl FnOnce(&mut Tracker) -> tracker::Result<T>) -> CmdResult<T> {
    f(&mut state.tracker.lock().unwrap()).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_characters(state: State<'_, TrackerState>) -> CmdResult<Vec<Character>> {
    with(&state, |t| t.characters())
}

#[tauri::command]
pub fn add_character(state: State<'_, TrackerState>, name: String, class: Option<String>) -> CmdResult<Character> {
    with(&state, |t| t.add_character(&name, class.as_deref(), now()))
}

#[tauri::command]
pub fn update_character(state: State<'_, TrackerState>, id: i64, name: String, class: Option<String>) -> CmdResult<()> {
    with(&state, |t| t.update_character(id, &name, class.as_deref()))
}

#[tauri::command]
pub fn remove_character(state: State<'_, TrackerState>, id: i64) -> CmdResult<()> {
    with(&state, |t| t.remove_character(id))
}

#[tauri::command]
pub fn get_checklist(state: State<'_, TrackerState>, character_id: i64) -> CmdResult<Checklist> {
    with(&state, |t| t.checklist(character_id, now()))
}

#[tauri::command]
pub fn set_task_count(state: State<'_, TrackerState>, character_id: i64, task_id: String, count: u32) -> CmdResult<()> {
    with(&state, |t| t.set_task_count(character_id, &task_id, count, now()))
}

#[tauri::command]
pub fn add_custom_task(state: State<'_, TrackerState>, name: String, period: Period, goal: u32) -> CmdResult<String> {
    with(&state, |t| t.add_custom_task(&name, period, goal))
}

#[tauri::command]
pub fn remove_custom_task(state: State<'_, TrackerState>, id: String) -> CmdResult<()> {
    with(&state, |t| t.remove_custom_task(&id))
}

#[tauri::command]
pub fn get_pets(state: State<'_, TrackerState>, character_id: i64) -> CmdResult<Vec<PetStatus>> {
    with(&state, |t| t.pets(character_id))
}

#[tauri::command]
pub fn set_pet_owned(state: State<'_, TrackerState>, character_id: i64, pet_id: String, owned: bool) -> CmdResult<()> {
    with(&state, |t| t.set_pet_owned(character_id, &pet_id, owned, now()))
}

#[tauri::command]
pub fn add_custom_pet(
    state: State<'_, TrackerState>,
    name: String,
    group: String,
    origin: String,
) -> CmdResult<String> {
    with(&state, |t| t.add_custom_pet(&name, &group, &origin))
}

#[tauri::command]
pub fn remove_custom_pet(state: State<'_, TrackerState>, id: String) -> CmdResult<()> {
    with(&state, |t| t.remove_custom_pet(&id))
}

#[tauri::command]
pub fn list_events(state: State<'_, TrackerState>) -> CmdResult<Vec<EventConfig>> {
    with(&state, |t| t.events(now()))
}

#[tauri::command]
pub fn upcoming_events(state: State<'_, TrackerState>, hours: Option<u32>) -> CmdResult<Vec<UpcomingEvent>> {
    with(&state, |t| t.upcoming(now(), hours.unwrap_or(24).min(24 * 8)))
}

#[tauri::command]
pub fn set_event_alert(
    state: State<'_, TrackerState>,
    event_id: String,
    enabled: bool,
    lead_min: u32,
) -> CmdResult<()> {
    with(&state, |t| t.set_alert(&event_id, enabled, lead_min))?;
    let _ = state.wake.send(());
    Ok(())
}

#[tauri::command]
pub fn test_notification(app: AppHandle) -> CmdResult<()> {
    app.notification()
        .builder()
        .title("Aion 2 Assistente")
        .body("As notificações de eventos estão funcionando.")
        .show()
        .map_err(|e| e.to_string())
}

fn notify(app: &AppHandle, a: &Alert, now_ms: i64) {
    let mins = (a.start_ms - now_ms + 30_000).div_euclid(60_000);
    let when = if mins <= 0 {
        "Começando agora.".to_string()
    } else {
        format!("Começa em {mins} min.")
    };
    let body = match &a.descricao {
        Some(d) => format!("{when} {d}"),
        None => when,
    };
    if let Err(e) = app.notification().builder().title(&a.nome).body(body).show() {
        tracing::warn!("notificação falhou: {e}");
    }
    let _ = app.emit("alert:fire", a);
}

/// Dorme até o próximo alerta (no máximo 1 min, para seguir mudanças no
/// relógio do PC) e dispara os que venceram desde a última volta.
pub fn spawn_alerts(app: AppHandle, tracker: Arc<Mutex<Tracker>>) -> Sender<()> {
    let (tx, rx) = mpsc::channel::<()>();
    std::thread::Builder::new()
        .name("alertas".into())
        .spawn(move || {
            let mut last = now();
            loop {
                let next = tracker.lock().unwrap().next_alert_ms(last).ok().flatten();
                let wait = next.map_or(60_000, |n| (n - now()).clamp(0, 60_000));
                if let Err(RecvTimeoutError::Disconnected) = rx.recv_timeout(Duration::from_millis(wait as u64)) {
                    return;
                }
                let current = now();
                if current < last {
                    // Relógio do PC voltou: recomeça daqui sem repetir alertas.
                    last = current;
                    continue;
                }
                let due = tracker.lock().unwrap().due_alerts(last, current).unwrap_or_default();
                last = current;
                for a in due {
                    // Depois de o PC hibernar, não avisa de eventos que já começaram.
                    if current <= a.start_ms + 60_000 {
                        notify(&app, &a, current);
                    }
                }
            }
        })
        .expect("thread de alertas");
    tx
}
