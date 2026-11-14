//! Claude Code status in the island (PLAN.md stage 6): Claude Code hooks POST their
//! events to a local endpoint; the island shows working / waiting sessions.

mod hooks;
mod http;
mod model;

use std::io::Write;
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use serde::Serialize;
use serde_json::Value;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::error::{AppError, AppResult};
use crate::thread::spawn_supervised;
use crate::util::now_ms;
use model::{HookEvent, Session, Sessions};

const READ_TIMEOUT: Duration = Duration::from_secs(2);
const EXPIRE_EVERY: Duration = Duration::from_secs(60);

#[derive(Default)]
pub struct ClaudeShared {
    sessions: Mutex<Sessions>,
    /// Set when the port could not be opened (another app uses it).
    server_error: Mutex<Option<String>>,
}

type Shared = Arc<ClaudeShared>;

fn publish(app: &AppHandle, shared: &ClaudeShared) {
    let list = shared
        .sessions
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .list()
        .to_vec();
    if let Err(err) = app.emit("claude://status", list) {
        log::warn!("failed to emit claude://status: {err}");
    }
}

fn respond(mut stream: &TcpStream, status: &str) {
    let _ = write!(
        stream,
        "HTTP/1.1 {status}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
    );
}

fn handle(app: &AppHandle, shared: &ClaudeShared, stream: &TcpStream) {
    let _ = stream.set_read_timeout(Some(READ_TIMEOUT));
    let body = match http::read_hook_request(stream) {
        Ok(body) => body,
        Err(rejection) => {
            respond(stream, rejection.status_line());
            return;
        }
    };
    let Ok(event) = serde_json::from_slice::<HookEvent>(&body) else {
        respond(stream, http::Rejection::BadRequest.status_line());
        return;
    };
    respond(stream, "204 No Content");

    let session_id = event.session_id.clone();
    let attention = shared
        .sessions
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .apply(event, now_ms());
    publish(app, shared);
    if attention {
        let session = shared
            .sessions
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .list()
            .iter()
            .find(|s| s.id == session_id)
            .cloned();
        if let Err(err) = app.emit("claude://attention", session) {
            log::warn!("failed to emit claude://attention: {err}");
        }
    }
}

/// Starts the local endpoint and the expiry timer.
pub fn start(app: &AppHandle) {
    let shared: Shared = Arc::default();
    app.manage(shared.clone());

    match TcpListener::bind(("127.0.0.1", hooks::PORT)) {
        Ok(listener) => {
            log::info!("Claude Code endpoint on 127.0.0.1:{}", hooks::PORT);
            let (app, shared) = (app.clone(), shared.clone());
            let listener = Arc::new(listener);
            spawn_supervised("claude-endpoint", move || {
                for stream in listener.incoming().flatten() {
                    handle(&app, &shared, &stream);
                }
            });
        }
        Err(err) => {
            log::warn!("Claude Code endpoint unavailable: {err}");
            *shared
                .server_error
                .lock()
                .unwrap_or_else(PoisonError::into_inner) =
                Some(format!("порт {} занят другой программой", hooks::PORT));
        }
    }

    let app = app.clone();
    spawn_supervised("claude-expiry", move || loop {
        std::thread::sleep(EXPIRE_EVERY);
        let changed = shared
            .sessions
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .expire(now_ms());
        if changed {
            publish(&app, &shared);
        }
    });
}

#[tauri::command]
pub fn claude_status(shared: State<'_, Shared>) -> AppResult<Vec<Session>> {
    Ok(shared
        .sessions
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .list()
        .to_vec())
}

/// Claude Code's user settings file (honours `CLAUDE_CONFIG_DIR`).
fn settings_path(app: &AppHandle) -> AppResult<PathBuf> {
    let dir = match std::env::var_os("CLAUDE_CONFIG_DIR") {
        Some(dir) => PathBuf::from(dir),
        None => app.path().home_dir()?.join(".claude"),
    };
    Ok(dir.join("settings.json"))
}

fn read_settings(path: &PathBuf) -> AppResult<Value> {
    match std::fs::read_to_string(path) {
        Ok(text) if text.trim().is_empty() => Ok(Value::Object(Default::default())),
        Ok(text) => serde_json::from_str(&text).map_err(|err| {
            AppError::InvalidArgument(format!(
                "не удалось разобрать {}: {err}. Файл не изменён",
                path.display()
            ))
        }),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            Ok(Value::Object(Default::default()))
        }
        Err(err) => Err(err.into()),
    }
}

/// Writes the file, keeping a one-time backup of the original next to it.
fn write_settings(path: &PathBuf, settings: &Value) -> AppResult<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let backup = path.with_extension("json.escape-backup");
    if path.exists() && !backup.exists() {
        std::fs::copy(path, &backup)?;
    }
    let text = serde_json::to_string_pretty(settings)
        .map_err(|err| AppError::Internal(err.to_string()))?;
    std::fs::write(path, text + "\n")?;
    Ok(())
}

/// Mirrored in `src/ipc/commands.ts`.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HooksStatus {
    pub installed: bool,
    pub settings_path: String,
    /// The local endpoint could not start.
    pub error: Option<String>,
}

#[tauri::command]
pub fn claude_hooks_status(app: AppHandle, shared: State<'_, Shared>) -> AppResult<HooksStatus> {
    let path = settings_path(&app)?;
    let installed = read_settings(&path).is_ok_and(|s| hooks::is_installed(&s));
    Ok(HooksStatus {
        installed,
        settings_path: path.display().to_string(),
        error: shared
            .server_error
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone(),
    })
}

/// Adds (`true`) or removes (`false`) Escape's hooks in Claude Code's settings.
#[tauri::command]
pub fn claude_hooks_set(
    enabled: bool,
    app: AppHandle,
    shared: State<'_, Shared>,
) -> AppResult<HooksStatus> {
    let path = settings_path(&app)?;
    let mut settings = read_settings(&path)?;
    if enabled {
        hooks::install(&mut settings).map_err(|msg| AppError::InvalidArgument(msg.into()))?;
    } else {
        hooks::uninstall(&mut settings);
    }
    write_settings(&path, &settings)?;
    log::info!(
        "Claude Code hooks {} in {}",
        if enabled { "installed" } else { "removed" },
        path.display()
    );
    claude_hooks_status(app, shared)
}
