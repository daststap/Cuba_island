//! Downloads in progress (PLAN.md stage 6): browsers (growing temporary files
//! in the Downloads folder) and Steam (app manifests). Both are read by one
//! background loop — browsers every second, Steam every two — and the frontend
//! gets the list on change plus an event when something finishes.

mod browser;
mod steam;
#[cfg(windows)]
mod win32;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, SystemTime};

use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::error::{AppError, AppResult};
use crate::thread::spawn_supervised;
use crate::util::now_ms;
use browser::{BrowserTracker, Partial};
use steam::{AppManifest, SteamTracker};

const TICK: Duration = Duration::from_secs(1);
/// Steam is read every this many ticks.
const STEAM_EVERY: u64 = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Source {
    Browser,
    Steam,
}

/// One download in progress. Mirrored in `src/ipc/commands.ts`.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Download {
    pub id: String,
    pub source: Source,
    pub name: String,
    pub received: u64,
    /// `None` when unknown (browsers do not reveal it).
    pub total: Option<u64>,
    /// Bytes per second, smoothed; `None` until measured.
    pub speed: Option<f64>,
}

/// A finished download. Mirrored in `src/ipc/commands.ts`.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Finished {
    pub source: Source,
    pub name: String,
    /// The file, for "show in folder" (browser downloads only).
    pub path: Option<String>,
}

static CURRENT: Mutex<Vec<Download>> = Mutex::new(Vec::new());

fn emit<P: Serialize + Clone>(app: &AppHandle, event: &str, payload: P) {
    if let Err(err) = app.emit(event, payload) {
        log::warn!("failed to emit {event}: {err}");
    }
}

/// Temporary download files in `dir` with their sizes and last writes.
fn unix_ms(time: Option<SystemTime>) -> i64 {
    time.and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
        .map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
}

/// The file a finished download became: its own name without the temporary
/// suffix, or — when the browser renamed it ("Не подтверждено 123456.~") —
/// the file with the same creation time. `None` if it was cancelled.
fn resolve_finished(dir: &Path, temp: &str, created: i64) -> Option<String> {
    let has_content = |meta: &std::fs::Metadata| meta.is_file() && meta.len() > 0;
    if let Some(name) = browser::final_name(temp).filter(|n| !browser::is_unconfirmed(n)) {
        if std::fs::metadata(dir.join(name)).is_ok_and(|m| has_content(&m)) {
            return Some(name.to_owned());
        }
    }
    std::fs::read_dir(dir).ok()?.flatten().find_map(|entry| {
        let name = entry.file_name().to_string_lossy().into_owned();
        if browser::final_name(&name).is_some() {
            return None;
        }
        let meta = std::fs::metadata(entry.path()).ok()?;
        (has_content(&meta) && unix_ms(meta.created().ok()) == created).then_some(name)
    })
}

fn partials(dir: &Path) -> Vec<Partial> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter_map(|entry| {
            let file = entry.file_name().to_string_lossy().into_owned();
            browser::final_name(&file)?;
            // Not `entry.metadata()`: on Windows that is the directory listing's
            // copy, which lags behind for files a browser keeps open.
            let meta = std::fs::metadata(entry.path())
                .ok()
                .filter(|m| m.is_file())?;
            Some(Partial {
                file,
                size: meta.len(),
                modified: unix_ms(meta.modified().ok()),
                created: unix_ms(meta.created().ok()),
            })
        })
        .collect()
}

/// Manifests of every Steam library, re-parsing only files that changed.
struct SteamReader {
    root: PathBuf,
    cache: HashMap<PathBuf, (SystemTime, Option<AppManifest>)>,
}

impl SteamReader {
    fn manifests(&mut self) -> Vec<AppManifest> {
        let vdf = std::fs::read_to_string(self.root.join("steamapps").join("libraryfolders.vdf"))
            .unwrap_or_default();
        let mut libraries: Vec<PathBuf> = steam::library_paths(&vdf)
            .into_iter()
            .map(PathBuf::from)
            .collect();
        if libraries.is_empty() {
            libraries.push(self.root.clone());
        }
        let mut seen = HashMap::new();
        for library in libraries {
            let Ok(entries) = std::fs::read_dir(library.join("steamapps")) else {
                continue;
            };
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().into_owned();
                if !(name.starts_with("appmanifest_") && name.ends_with(".acf")) {
                    continue;
                }
                let path = entry.path();
                let Ok(modified) = entry.metadata().and_then(|m| m.modified()) else {
                    continue;
                };
                let parsed = match self.cache.remove(&path) {
                    Some((time, parsed)) if time == modified => parsed,
                    _ => std::fs::read_to_string(&path)
                        .ok()
                        .and_then(|text| steam::parse_manifest(&text)),
                };
                seen.insert(path, (modified, parsed));
            }
        }
        self.cache = seen;
        self.cache
            .values()
            .filter_map(|(_, parsed)| parsed.clone())
            .collect()
    }
}

#[cfg(windows)]
fn folders() -> (Option<PathBuf>, Option<PathBuf>) {
    (win32::downloads_dir(), win32::steam_dir())
}

#[cfg(not(windows))]
fn folders() -> (Option<PathBuf>, Option<PathBuf>) {
    (None, None)
}

fn watch(app: &AppHandle) {
    let (downloads, steam_root) = folders();
    log::info!("watching downloads in {downloads:?}, Steam in {steam_root:?}");
    let mut browser = BrowserTracker::default();
    let mut steam_tracker = SteamTracker::default();
    let mut steam_reader = steam_root.map(|root| SteamReader {
        root,
        cache: HashMap::new(),
    });
    let mut steam_list = Vec::new();
    // Temporary files already logged once.
    let mut seen_files = std::collections::HashSet::new();
    let mut tick = 0u64;
    loop {
        tick = tick.wrapping_add(1);
        let now = now_ms();
        let mut list = Vec::new();
        if let Some(dir) = &downloads {
            let found = partials(dir);
            for p in &found {
                if seen_files.insert(p.file.clone()) {
                    log::info!("temporary download file: {} ({} bytes)", p.file, p.size);
                }
            }
            seen_files.retain(|file| found.iter().any(|p| &p.file == file));
            let (active, finished) = browser.update(now, found, |temp, created| {
                resolve_finished(dir, temp, created)
            });
            list.extend(active);
            for name in finished {
                let path = dir.join(&name).display().to_string();
                log::info!("download finished: {name}");
                emit(
                    app,
                    "downloads://finished",
                    Finished {
                        source: Source::Browser,
                        name,
                        path: Some(path),
                    },
                );
            }
        }
        if let Some(reader) = steam_reader.as_mut().filter(|_| tick % STEAM_EVERY == 0) {
            let (active, finished) = steam_tracker.update(now, &reader.manifests());
            steam_list = active;
            for name in finished {
                log::info!("Steam finished: {name}");
                emit(
                    app,
                    "downloads://finished",
                    Finished {
                        source: Source::Steam,
                        name,
                        path: None,
                    },
                );
            }
        }
        list.extend(steam_list.iter().cloned());

        let mut current = CURRENT.lock().unwrap_or_else(PoisonError::into_inner);
        if *current != list {
            *current = list.clone();
            drop(current);
            emit(app, "downloads://changed", list);
        }
        std::thread::sleep(TICK);
    }
}

pub fn start(app: &AppHandle) {
    let app = app.clone();
    spawn_supervised("downloads", move || watch(&app));
}

#[tauri::command]
pub fn downloads_list() -> AppResult<Vec<Download>> {
    Ok(CURRENT
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clone())
}

/// Opens Explorer with the downloaded file selected.
#[tauri::command]
pub fn downloads_reveal(path: String) -> AppResult<()> {
    if !Path::new(&path).is_file() {
        return Err(AppError::Unavailable(
            "файл уже перемещён или удалён".into(),
        ));
    }
    let mut command = std::process::Command::new("explorer.exe");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // Explorer wants exactly `/select,"path"`; regular quoting would wrap the comma too.
        command.raw_arg(format!("/select,\"{path}\""));
    }
    #[cfg(not(windows))]
    command.arg(&path);
    command.spawn()?;
    Ok(())
}
