//! Atualização automática: ao abrir, procura uma versão nova nas GitHub
//! Releases (latest.json). Se houver, baixa, instala e o app reabre sozinho.

use std::sync::Mutex;

use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_updater::UpdaterExt;

/// Versão sendo instalada agora, para a UI mostrar o aviso mesmo se abrir
/// depois do evento `update:installing`.
#[derive(Default)]
pub struct UpdateState(pub Mutex<Option<String>>);

#[tauri::command]
pub fn update_installing(state: tauri::State<'_, UpdateState>) -> Option<String> {
    state.0.lock().ok().and_then(|v| v.clone())
}

pub fn spawn_check(app: AppHandle) {
    // Em desenvolvimento não há instalador para trocar.
    if cfg!(debug_assertions) {
        return;
    }
    tauri::async_runtime::spawn(async move {
        if let Err(e) = check_and_install(&app).await {
            // Sem internet ou sem release nova: segue com a versão atual.
            tracing::warn!("atualização automática: {e}");
        }
    });
}

async fn check_and_install(app: &AppHandle) -> tauri_plugin_updater::Result<()> {
    let Some(update) = app.updater()?.check().await? else {
        tracing::info!("app já está na versão mais recente");
        return Ok(());
    };
    tracing::info!("instalando a versão {}", update.version);
    if let Ok(mut v) = app.state::<UpdateState>().0.lock() {
        *v = Some(update.version.clone());
    }
    let _ = app.emit("update:installing", &update.version);
    update.download_and_install(|_, _| {}, || {}).await?;
    // No Windows o instalador já fecha o app e o reabre; isto cobre o resto.
    app.restart();
}
