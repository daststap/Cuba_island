//! Now playing via Global System Media Transport Controls (PLAN.md §5, stage 2).

#[cfg(windows)]
mod cover;
#[cfg(windows)]
mod gsmtc;
mod model;

use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use tauri::{AppHandle, Manager, State};

use crate::error::{AppError, AppResult};
pub use model::{MediaCommand, MediaState};

const COMMAND_TIMEOUT: Duration = Duration::from_secs(3);

/// Messages for the media worker thread.
enum Msg {
    /// Something changed in GSMTC; re-read the state (debounced).
    #[cfg_attr(not(windows), allow(dead_code))]
    Changed,
    Command(MediaCommand, Sender<AppResult<()>>),
}

pub struct MediaShared {
    state: Mutex<Option<MediaState>>,
    tx: Mutex<Sender<Msg>>,
}

/// Starts the media worker; its state is served by [`media_get_state`].
pub fn start(app: &AppHandle) {
    let (tx, rx) = mpsc::channel();
    let shared = Arc::new(MediaShared {
        state: Mutex::new(None),
        tx: Mutex::new(tx.clone()),
    });
    app.manage(shared.clone());

    #[cfg(windows)]
    {
        use tauri::Emitter;

        const RETRY_DELAY: Duration = Duration::from_secs(5);
        let app = app.clone();
        crate::thread::spawn_supervised("media", move || {
            let publish = |state: Option<MediaState>| {
                *shared.state.lock().unwrap_or_else(PoisonError::into_inner) = state.clone();
                if let Err(err) = app.emit("media://changed", state) {
                    log::warn!("failed to emit media://changed: {err}");
                }
            };
            while let Err(err) = gsmtc::run(&tx, &rx, &publish) {
                log::error!("media worker failed: {err}; retrying");
                publish(None);
                std::thread::sleep(RETRY_DELAY);
            }
        });
    }
    #[cfg(not(windows))]
    drop((rx, shared));
}

#[tauri::command]
pub fn media_get_state(shared: State<'_, Arc<MediaShared>>) -> AppResult<Option<MediaState>> {
    Ok(shared
        .state
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clone())
}

#[tauri::command]
pub async fn media_command(
    cmd: MediaCommand,
    shared: State<'_, Arc<MediaShared>>,
) -> AppResult<()> {
    let (reply_tx, reply_rx) = mpsc::channel();
    shared
        .tx
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .send(Msg::Command(cmd, reply_tx))
        .map_err(|_| AppError::Unavailable("Модуль музыки не запущен".into()))?;

    tauri::async_runtime::spawn_blocking(move || reply_rx.recv_timeout(COMMAND_TIMEOUT))
        .await
        .map_err(|err| AppError::Internal(err.to_string()))?
        .map_err(|_| AppError::Unavailable("Плеер не ответил".into()))?
}
