// Sem janela de console no Windows em release.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod npcap;
mod runtime;
mod tracker_cmds;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

/// Estado atual do overlay: `true` = cliques atravessam a janela.
static CLICK_THROUGH: AtomicBool = AtomicBool::new(true);

pub fn set_click_through(app: &AppHandle, enabled: bool) -> tauri::Result<()> {
    if let Some(w) = app.get_webview_window("overlay") {
        w.set_ignore_cursor_events(enabled)?;
        CLICK_THROUGH.store(enabled, Ordering::Relaxed);
        let _ = app.emit("overlay:click_through", enabled);
    }
    Ok(())
}

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();

    // Antes de qualquer chamada ao pcap (wpcap.dll é carregada sob demanda).
    let npcap_ok = npcap::prepare();

    // Ctrl+Shift+O: editar overlay (mover/redimensionar). Ctrl+Shift+R: nova luta.
    let edit_overlay = Shortcut::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::KeyO);
    let reset_fight = Shortcut::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::KeyR);

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(move |app, shortcut, event| {
                    if event.state() != ShortcutState::Pressed {
                        return;
                    }
                    if shortcut == &edit_overlay {
                        let next = !CLICK_THROUGH.load(Ordering::Relaxed);
                        let _ = set_click_through(app, next);
                    } else if shortcut == &reset_fight {
                        let state = app.state::<commands::AppState>();
                        let _ = state.runtime.cmd_tx.send(runtime::Command::Reset);
                    }
                })
                .build(),
        )
        .setup(move |app| {
            let data_dir = app.path().app_data_dir()?;
            let store = storage::Store::open(&data_dir.join("data.db"))?;
            // Retenção padrão do histórico: 90 dias.
            let _ = store.prune_before(runtime::now_ms().saturating_sub(90 * 24 * 3600 * 1000));
            let store = Arc::new(Mutex::new(store));
            let rt = runtime::spawn(app.handle().clone(), store.clone(), npcap_ok);
            // Desenvolvimento: `aion2-assistente --replay gravacao.pcap`.
            let args: Vec<String> = std::env::args().collect();
            if let Some(path) = args.iter().position(|a| a == "--replay").and_then(|i| args.get(i + 1)) {
                let _ = rt.cmd_tx.send(runtime::Command::Replay(path.into()));
            }
            app.manage(commands::AppState { runtime: rt, store });

            let tracker = Arc::new(Mutex::new(tracker::Tracker::open(&data_dir.join("tracker.db"))?));
            let wake = tracker_cmds::spawn_alerts(app.handle().clone(), tracker.clone());
            app.manage(tracker_cmds::TrackerState { tracker, wake });

            app.global_shortcut().register(edit_overlay)?;
            app.global_shortcut().register(reset_fight)?;
            set_click_through(app.handle(), true)?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_status,
            commands::reset_encounter,
            commands::list_encounters,
            commands::get_encounter,
            commands::replay_file,
            commands::set_recording,
            commands::set_overlay_click_through,
            commands::toggle_overlay,
            tracker_cmds::list_characters,
            tracker_cmds::add_character,
            tracker_cmds::update_character,
            tracker_cmds::remove_character,
            tracker_cmds::get_checklist,
            tracker_cmds::set_task_count,
            tracker_cmds::add_custom_task,
            tracker_cmds::remove_custom_task,
            tracker_cmds::get_pets,
            tracker_cmds::set_pet_owned,
            tracker_cmds::add_custom_pet,
            tracker_cmds::remove_custom_pet,
            tracker_cmds::list_events,
            tracker_cmds::upcoming_events,
            tracker_cmds::set_event_alert,
            tracker_cmds::test_notification,
        ])
        .run(tauri::generate_context!())
        .expect("erro ao iniciar o app");
}
