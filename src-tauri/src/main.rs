// Sem janela de console no Windows em release.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod npcap;
mod official;
mod runtime;
mod tracker_cmds;
mod updater;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, WindowEvent};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};
use tauri_plugin_window_state::StateFlags;

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

/// Traz a janela principal de volta (da bandeja ou de um segundo clique no atalho).
fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}

/// Ícone na bandeja do Windows: clique abre o app; o menu tem Abrir,
/// overlay e Sair. Fechar a janela só a esconde, para o medidor e os
/// alertas continuarem rodando.
fn build_tray(app: &tauri::App) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "abrir", "Abrir", true, None::<&str>)?;
    let overlay = MenuItem::with_id(app, "overlay", "Mostrar/ocultar overlay", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "sair", "Sair", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &overlay, &quit])?;
    let mut tray = TrayIconBuilder::with_id("bandeja")
        .tooltip("Aion 2 Assistente")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "abrir" => show_main(app),
            "overlay" => {
                let _ = commands::toggle_overlay(app.clone());
            }
            "sair" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }
    tray.build(app)?;
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
        // Abrir o atalho de novo com o app na bandeja só mostra a janela.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| show_main(app)))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(updater::UpdateState::default())
        // Lembra onde o usuário deixou o overlay e a janela principal.
        .plugin(
            tauri_plugin_window_state::Builder::new()
                .with_state_flags(StateFlags::POSITION | StateFlags::SIZE)
                .build(),
        )
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
            build_tray(app)?;
            updater::spawn_check(app.handle().clone());
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "main" {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_status,
            commands::reset_encounter,
            commands::list_encounters,
            commands::get_encounter,
            commands::replay_file,
            commands::set_recording,
            commands::recordings_dir,
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
            official::official_search,
            official::official_link,
            official::official_refresh,
            official::official_cached,
            official::official_fetch,
            updater::update_installing,
        ])
        .run(tauri::generate_context!())
        .expect("erro ao iniciar o app");
}
