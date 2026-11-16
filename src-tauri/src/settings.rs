//! User settings (PLAN.md stage 5): stored with `tauri-plugin-store`, applied live.

use std::sync::{Mutex, PoisonError};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_autostart::ManagerExt as _;
use tauri_plugin_store::StoreExt;

use crate::error::{AppError, AppResult};
use crate::{clipboard, hotkeys, window};

const STORE_FILE: &str = "settings.json";
const STORE_KEY: &str = "settings";

/// Mirrored in `src/ipc/commands.ts`. Unknown or missing fields fall back to defaults,
/// so older settings files keep working after updates.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    /// Monitor device name; `None` = primary.
    pub monitor: Option<String>,
    pub autostart: bool,
    pub hide_in_fullscreen: bool,
    pub hour12: bool,
    pub clipboard_hotkey: String,
    pub launcher_hotkey: String,
    pub screenshot_hotkey: String,
    pub history_limit: u32,
    pub copy_peek: bool,
    /// Orange/green dots while the microphone/camera is in use.
    pub privacy_dots: bool,
    /// A short peek when Caps Lock is toggled.
    pub caps_peek: bool,
    /// Downloads in the pill and on the home tab.
    pub downloads: bool,
    /// Island background opacity, 0.5–1.
    pub opacity: f64,
    /// The first-run autostart question has been answered.
    pub onboarded: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            monitor: None,
            autostart: false,
            hide_in_fullscreen: true,
            hour12: false,
            clipboard_hotkey: hotkeys::DEFAULT_CLIPBOARD.to_owned(),
            launcher_hotkey: hotkeys::DEFAULT_LAUNCHER.to_owned(),
            screenshot_hotkey: hotkeys::DEFAULT_SCREENSHOT.to_owned(),
            history_limit: 500,
            copy_peek: true,
            privacy_dots: true,
            caps_peek: true,
            downloads: true,
            opacity: 1.0,
            onboarded: false,
        }
    }
}

pub const HISTORY_LIMITS: std::ops::RangeInclusive<u32> = 50..=5000;
pub const OPACITY: std::ops::RangeInclusive<f64> = 0.5..=1.0;

impl Settings {
    pub fn hotkeys(&self) -> hotkeys::Keys<'_> {
        hotkeys::Keys {
            clipboard: &self.clipboard_hotkey,
            launcher: &self.launcher_hotkey,
            screenshot: &self.screenshot_hotkey,
        }
    }

    fn validate(&self) -> AppResult<()> {
        if !HISTORY_LIMITS.contains(&self.history_limit) {
            return Err(AppError::InvalidArgument(format!(
                "лимит истории должен быть от {} до {}",
                HISTORY_LIMITS.start(),
                HISTORY_LIMITS.end()
            )));
        }
        if !OPACITY.contains(&self.opacity) {
            return Err(AppError::InvalidArgument(
                "прозрачность должна быть от 50 до 100%".into(),
            ));
        }
        Ok(())
    }

    /// Applies a partial update (`{ "hour12": true }`); unknown keys are rejected.
    fn merged(&self, patch: &Value) -> AppResult<Self> {
        let Value::Object(changes) = patch else {
            return Err(AppError::InvalidArgument("ожидался объект настроек".into()));
        };
        let mut current =
            serde_json::to_value(self).map_err(|err| AppError::Internal(err.to_string()))?;
        let Value::Object(fields) = &mut current else {
            return Err(AppError::Internal("настройки не объект".into()));
        };
        for (key, value) in changes {
            if !fields.contains_key(key) {
                return Err(AppError::InvalidArgument(format!(
                    "неизвестная настройка {key}"
                )));
            }
            fields.insert(key.clone(), value.clone());
        }
        let next: Self = serde_json::from_value(current)
            .map_err(|err| AppError::InvalidArgument(format!("неверное значение: {err}")))?;
        next.validate()?;
        Ok(next)
    }
}

pub type SettingsState = Mutex<Settings>;

/// Loads settings (defaults if missing or broken) and registers them as state.
pub fn load(app: &AppHandle) -> Settings {
    let stored = app
        .store(STORE_FILE)
        .ok()
        .and_then(|store| store.get(STORE_KEY))
        .and_then(|value| serde_json::from_value::<Settings>(value).ok())
        .filter(|s| s.validate().is_ok());
    let mut settings = stored.unwrap_or_default();
    // The system is the source of truth for autostart (the user may change it in Task Manager).
    if let Ok(enabled) = app.autolaunch().is_enabled() {
        settings.autostart = enabled;
        // Re-register so the entry points at this exe (e.g. after reinstalling elsewhere).
        if enabled {
            if let Err(err) = app.autolaunch().enable() {
                log::warn!("failed to refresh the autostart entry: {err}");
            }
        }
    }
    app.manage(Mutex::new(settings.clone()));
    settings
}

fn save(app: &AppHandle, settings: &Settings) -> AppResult<()> {
    let store = app
        .store(STORE_FILE)
        .map_err(|err| AppError::Internal(err.to_string()))?;
    store.set(
        STORE_KEY,
        serde_json::to_value(settings).map_err(|err| AppError::Internal(err.to_string()))?,
    );
    store
        .save()
        .map_err(|err| AppError::Internal(format!("не удалось сохранить настройки: {err}")))
}

/// Applies what changed between `old` and `new` to the running app.
fn apply(app: &AppHandle, old: &Settings, new: &Settings) -> AppResult<()> {
    if old.clipboard_hotkey != new.clipboard_hotkey
        || old.launcher_hotkey != new.launcher_hotkey
        || old.screenshot_hotkey != new.screenshot_hotkey
    {
        hotkeys::rebind(app, &new.hotkeys())?;
    }
    if old.autostart != new.autostart {
        let launcher = app.autolaunch();
        let result = if new.autostart {
            launcher.enable()
        } else {
            launcher.disable()
        };
        result.map_err(|err| AppError::Internal(format!("автозапуск: {err}")))?;
    }
    if old.monitor != new.monitor || old.hide_in_fullscreen != new.hide_in_fullscreen {
        window::apply_display(app, new.monitor.clone(), new.hide_in_fullscreen)?;
    }
    if old.history_limit != new.history_limit {
        clipboard::set_history_limit(app, new.history_limit as usize)?;
    }
    Ok(())
}

#[tauri::command]
pub fn settings_get(state: State<'_, SettingsState>) -> AppResult<Settings> {
    Ok(state.lock().unwrap_or_else(PoisonError::into_inner).clone())
}

/// Updates some settings; returns the full new settings. Nothing is saved if
/// applying fails (e.g. a hotkey is taken by another app).
#[tauri::command]
pub fn settings_set(
    patch: Value,
    app: AppHandle,
    state: State<'_, SettingsState>,
) -> AppResult<Settings> {
    let mut current = state.lock().unwrap_or_else(PoisonError::into_inner);
    let next = current.merged(&patch)?;
    apply(&app, &current, &next)?;
    save(&app, &next)?;
    *current = next.clone();
    drop(current);
    if let Err(err) = app.emit("settings://changed", &next) {
        log::warn!("failed to emit settings://changed: {err}");
    }
    Ok(next)
}

#[tauri::command]
pub fn monitors_list(app: AppHandle) -> AppResult<Vec<window::MonitorInfo>> {
    window::monitors(&app)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn merges_partial_updates() {
        let next = Settings::default()
            .merged(&json!({ "hour12": true, "historyLimit": 100 }))
            .unwrap();
        assert!(next.hour12);
        assert_eq!(next.history_limit, 100);
        assert_eq!(next.launcher_hotkey, "Alt+Space");
    }

    #[test]
    fn rejects_unknown_keys_bad_types_and_out_of_range() {
        let s = Settings::default();
        assert!(s.merged(&json!({ "nope": 1 })).is_err());
        assert!(s.merged(&json!({ "hour12": "yes" })).is_err());
        assert!(s.merged(&json!({ "historyLimit": 10 })).is_err());
        assert!(s.merged(&json!({ "opacity": 0.2 })).is_err());
        assert!(s.merged(&json!([1, 2])).is_err());
    }

    #[test]
    fn old_files_without_new_fields_load_with_defaults() {
        let s: Settings = serde_json::from_value(json!({ "hour12": true })).unwrap();
        assert!(s.hour12);
        assert!(s.hide_in_fullscreen);
        assert_eq!(s.history_limit, 500);
    }
}
