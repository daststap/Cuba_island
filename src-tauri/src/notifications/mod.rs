//! Windows notifications in the island (PLAN.md stage 6).
//!
//! The Notification Center is read every 1.5 s through `UserNotificationListener`
//! (no package or certificate needed on current Windows 11). New notifications
//! raise `notifications://new` (a peek); the list of what is in the center goes
//! out as `notifications://list`. Opening or dismissing one in the island also
//! removes it from the Notification Center. Text is never logged.

mod model;
#[cfg(windows)]
mod win32;

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::{Mutex, PoisonError};
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::error::{AppError, AppResult};
use crate::thread::spawn_supervised;
pub use model::Item;

const POLL: Duration = Duration::from_millis(1500);
/// How many notifications the home tab lists.
const LIST_LIMIT: usize = 5;

/// Whether Windows lets the island read notifications. Mirrored in `src/ipc/commands.ts`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Access {
    Allowed,
    Denied,
    Unspecified,
}

/// Mirrored in `src/ipc/commands.ts`.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NotificationsState {
    pub access: Access,
    /// In the Notification Center, newest first, without muted apps.
    pub items: Vec<Item>,
    /// Every app seen sending notifications (for the settings switches).
    pub apps: Vec<String>,
}

struct Shared {
    access: Access,
    /// Everything in the center at the last look (muted apps included).
    all: Vec<Item>,
    apps: BTreeSet<String>,
    /// App id → name of every app seen.
    known: BTreeMap<String, String>,
}

static STATE: Mutex<Shared> = Mutex::new(Shared {
    access: Access::Unspecified,
    all: Vec::new(),
    apps: BTreeSet::new(),
    known: BTreeMap::new(),
});

/// Apps whose Windows banners the island turned off (app id → name), kept on
/// disk so they can be given back later, even after a restart.
fn banners_file(app: &AppHandle) -> Option<PathBuf> {
    app.path()
        .app_data_dir()
        .ok()
        .map(|dir| dir.join("notification-banners.json"))
}

fn load_hidden(app: &AppHandle) -> BTreeMap<String, String> {
    banners_file(app)
        .and_then(|path| std::fs::read_to_string(path).ok())
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

fn save_hidden(app: &AppHandle, hidden: &BTreeMap<String, String>) {
    if let Some(path) = banners_file(app) {
        let text = serde_json::to_string(hidden).unwrap_or_default();
        if let Err(err) = std::fs::write(path, text) {
            log::warn!("failed to save hidden banners: {err}");
        }
    }
}

/// Applies the "hide Windows banners" option for the apps seen so far.
#[cfg(windows)]
fn sync_banners(app: &AppHandle, hidden: &mut BTreeMap<String, String>) {
    let settings = crate::settings::snapshot(app);
    let active = settings.notifications && settings.hide_windows_banners;
    let known = shared().known.clone();
    let (hide, restore) = model::banner_plan(
        &known,
        hidden,
        active,
        &settings.notifications_muted,
        win32::banner_off,
    );
    if hide.is_empty() && restore.is_empty() {
        return;
    }
    for (app_id, name) in hide {
        log::info!("hiding Windows banners of {name}");
        win32::set_banner(&app_id, false);
        hidden.insert(app_id, name);
    }
    for app_id in restore {
        log::info!("giving Windows banners back to {:?}", hidden.get(&app_id));
        win32::set_banner(&app_id, true);
        hidden.remove(&app_id);
    }
    save_hidden(app, hidden);
}

fn shared() -> std::sync::MutexGuard<'static, Shared> {
    STATE.lock().unwrap_or_else(PoisonError::into_inner)
}

fn muted(app: &AppHandle) -> (bool, Vec<String>) {
    let settings = crate::settings::snapshot(app);
    (settings.notifications, settings.notifications_muted)
}

fn snapshot(app: &AppHandle) -> NotificationsState {
    let (enabled, muted) = muted(app);
    let state = shared();
    NotificationsState {
        access: state.access,
        items: if enabled {
            model::visible(&state.all, &muted, LIST_LIMIT)
        } else {
            Vec::new()
        },
        apps: state.apps.iter().cloned().collect(),
    }
}

fn emit<P: Serialize + Clone>(app: &AppHandle, event: &str, payload: P) {
    if let Err(err) = app.emit(event, payload) {
        log::warn!("failed to emit {event}: {err}");
    }
}

/// Sends the current list (after a change in the center or in settings).
pub fn refresh(app: &AppHandle) {
    emit(app, "notifications://list", snapshot(app));
}

#[cfg(windows)]
fn watch(app: &AppHandle) {
    let icons_dir = app
        .path()
        .app_cache_dir()
        .map(|dir| dir.join("icons"))
        .unwrap_or_default();
    let mut icons = win32::Icons::new(icons_dir);
    let mut tracker = model::Tracker::default();
    let mut hidden = load_hidden(app);
    loop {
        let access = win32::access();
        let changed_access = {
            let mut state = shared();
            let changed = state.access != access;
            state.access = access;
            changed
        };
        if changed_access {
            log::info!("notification access: {access:?}");
            refresh(app);
        }
        if access == Access::Allowed {
            match win32::current(&mut icons) {
                Ok(items) => {
                    let fresh = tracker.arrived(&items);
                    let changed = {
                        let mut state = shared();
                        state.apps.extend(items.iter().map(|i| i.app.clone()));
                        for item in &items {
                            if !item.app_id.is_empty() {
                                state.known.insert(item.app_id.clone(), item.app.clone());
                            }
                        }
                        let ids = |list: &[Item]| list.iter().map(|i| i.id).collect::<Vec<_>>();
                        let changed = ids(&state.all) != ids(&items);
                        state.all = items;
                        changed
                    };
                    let (enabled, muted) = muted(app);
                    for item in fresh {
                        if enabled && model::allowed(&item.app, &muted) {
                            log::info!("notification from {}", item.app);
                            emit(app, "notifications://new", item);
                        }
                    }
                    if changed {
                        refresh(app);
                    }
                }
                Err(err) => log::warn!("could not read notifications: {err}"),
            }
        }
        sync_banners(app, &mut hidden);
        std::thread::sleep(POLL);
    }
}

pub fn start(app: &AppHandle) {
    #[cfg(windows)]
    {
        if win32::access() == Access::Unspecified {
            // Windows asks the user once; the answer lands in the next poll.
            let _ = app.run_on_main_thread(|| {
                win32::request_access(|answer| {
                    log::info!("notification access answer: {answer:?}")
                });
            });
        }
        let app = app.clone();
        spawn_supervised("notifications", move || watch(&app));
    }
    #[cfg(not(windows))]
    let _ = app;
}

#[tauri::command]
pub fn notifications_get(app: AppHandle) -> AppResult<NotificationsState> {
    Ok(snapshot(&app))
}

/// Opens the app that sent it and removes it from the Notification Center.
#[tauri::command]
pub fn notifications_open(id: u32, app: AppHandle) -> AppResult<()> {
    #[cfg(windows)]
    {
        let app_id = win32::app_id(id)
            .ok_or_else(|| AppError::Unavailable("уведомление уже закрыто".into()))?;
        crate::window::hand_over_focus(&app)?;
        win32::open_app(&app_id)?;
        win32::remove(id);
        forget(&app, id);
    }
    #[cfg(not(windows))]
    let _ = (id, app);
    Ok(())
}

/// Removes it from the island and the Notification Center.
#[tauri::command]
pub fn notifications_dismiss(id: u32, app: AppHandle) -> AppResult<()> {
    #[cfg(windows)]
    win32::remove(id);
    forget(&app, id);
    Ok(())
}

fn forget(app: &AppHandle, id: u32) {
    shared().all.retain(|item| item.id != id);
    refresh(app);
}

/// Opens Windows' privacy page where notification access is switched on.
#[tauri::command]
pub fn notifications_open_settings() -> AppResult<()> {
    std::process::Command::new("explorer.exe")
        .arg("ms-settings:privacy-notifications")
        .spawn()
        .map_err(|err| AppError::Internal(err.to_string()))?;
    Ok(())
}
