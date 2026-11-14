//! Signed auto-updates from GitHub Releases (PLAN.md stage 5).
//!
//! The only network request the app makes on its own (PLAN.md §6).

use serde::Serialize;
use tauri::AppHandle;
use tauri_plugin_updater::UpdaterExt;

use crate::error::{AppError, AppResult};

/// Mirrored in `src/ipc/commands.ts`.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub version: String,
    pub notes: Option<String>,
}

fn updater_error(err: tauri_plugin_updater::Error) -> AppError {
    AppError::Unavailable(format!("не удалось проверить обновления: {err}"))
}

/// Placeholder owner in `tauri.conf.json` until the GitHub repository exists.
const PLACEHOLDER: &str = "/OWNER/";

/// Whether the update endpoint points at a real repository. With the placeholder
/// we never contact GitHub: that account could belong to anyone.
fn updates_configured(app: &AppHandle) -> bool {
    app.config()
        .plugins
        .0
        .get("updater")
        .and_then(|updater| updater.get("endpoints"))
        .is_some_and(|endpoints| !endpoints.to_string().contains(PLACEHOLDER))
}

/// Returns the newer version if one is published, `None` if this is the latest.
#[tauri::command]
pub async fn update_check(app: AppHandle) -> AppResult<Option<UpdateInfo>> {
    if !updates_configured(&app) {
        log::info!("update check skipped: repository not configured yet");
        return Ok(None);
    }
    let update = app
        .updater()
        .map_err(updater_error)?
        .check()
        .await
        .map_err(updater_error)?;
    Ok(update.map(|u| UpdateInfo {
        version: u.version,
        notes: u.body,
    }))
}

/// Downloads, verifies the signature, installs and restarts.
#[tauri::command]
pub async fn update_install(app: AppHandle) -> AppResult<()> {
    let update = app
        .updater()
        .map_err(updater_error)?
        .check()
        .await
        .map_err(updater_error)?
        .ok_or_else(|| AppError::Unavailable("обновлений нет".into()))?;
    log::info!("installing update {}", update.version);
    update
        .download_and_install(|_, _| {}, || {})
        .await
        .map_err(|err| AppError::Internal(format!("не удалось установить обновление: {err}")))?;
    app.restart();
}
