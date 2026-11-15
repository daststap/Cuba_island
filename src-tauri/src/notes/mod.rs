//! Notes with reminders and a countdown timer (PLAN.md stage 6).
//!
//! Reminder times are parsed from the note text on the frontend (it has the local
//! calendar); Rust stores them as Unix ms and fires them from a 1 s scheduler.

mod store;
mod timer;

use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager, State};

use crate::error::AppResult;
use crate::storage::Db;
use crate::thread::spawn_supervised;
use crate::util::now_ms;
pub use store::Note;
use timer::Ringer;
pub use timer::TimerState;

const TICK: Duration = Duration::from_secs(1);

pub struct NotesShared {
    db: Db,
    timer: Mutex<TimerState>,
    /// Set while a finished timer is ringing.
    ringer: Mutex<Option<Ringer>>,
}

type Shared = Arc<NotesShared>;

impl NotesShared {
    fn timer(&self) -> std::sync::MutexGuard<'_, TimerState> {
        self.timer.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn ringer(&self) -> std::sync::MutexGuard<'_, Option<Ringer>> {
        self.ringer.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Whether the ringing timer should sound now (and forgets it once it is over).
    fn ring(&self, now: i64) -> bool {
        let mut ringer = self.ringer();
        match ringer.as_mut().map(|r| r.poll(now)) {
            Some(Some(sound)) => sound,
            Some(None) => {
                *ringer = None;
                false
            }
            None => false,
        }
    }
}

fn emit<P: serde::Serialize + Clone>(app: &AppHandle, event: &str, payload: P) {
    if let Err(err) = app.emit(event, payload) {
        log::warn!("failed to emit {event}: {err}");
    }
}

/// A short system sound for reminders and finished timers.
fn chime() {
    #[cfg(windows)]
    {
        use windows::Win32::System::Diagnostics::Debug::MessageBeep;
        use windows::Win32::UI::WindowsAndMessaging::MB_ICONASTERISK;
        // SAFETY: plays the user's "Asterisk" system sound; no pointers involved.
        let _ = unsafe { MessageBeep(MB_ICONASTERISK) };
    }
}

fn tick(app: &AppHandle, shared: &NotesShared) {
    let now = now_ms();
    match store::take_due(&shared.db.lock(), now) {
        Ok(due) => {
            for note in due {
                log::info!("reminder {} is due", note.id);
                chime();
                emit(app, "notes://reminder", note);
            }
            // A note list showing "reminded" flags may need a refresh.
        }
        Err(err) => log::error!("failed to check reminders: {err}"),
    }

    let finished = {
        let mut timer = shared.timer();
        match timer.finished(now) {
            Some(idle) => {
                *timer = idle;
                true
            }
            None => false,
        }
    };
    if finished {
        log::info!("timer finished");
        *shared.ringer() = Some(Ringer::start(now));
        emit(app, "notes://timer", TimerState::default());
        emit(app, "notes://timer-done", ());
    }
    if shared.ring(now) {
        chime();
    }
}

pub fn start(app: &AppHandle, db: Db) {
    let shared: Shared = Arc::new(NotesShared {
        db,
        timer: Mutex::new(TimerState::default()),
        ringer: Mutex::new(None),
    });
    app.manage(shared.clone());
    let app = app.clone();
    spawn_supervised("notes-scheduler", move || loop {
        tick(&app, &shared);
        std::thread::sleep(TICK);
    });
}

fn changed(app: &AppHandle) {
    emit(app, "notes://changed", ());
}

#[tauri::command]
pub fn notes_list(shared: State<'_, Shared>) -> AppResult<Vec<Note>> {
    store::list(&shared.db.lock())
}

/// `remind_at` is Unix ms parsed from the text by the frontend, or `None`.
#[tauri::command]
pub fn notes_add(
    text: String,
    remind_at: Option<i64>,
    app: AppHandle,
    shared: State<'_, Shared>,
) -> AppResult<Note> {
    let note = store::add(&shared.db.lock(), &text, remind_at, now_ms())?;
    changed(&app);
    Ok(note)
}

#[tauri::command]
pub fn notes_set_done(
    id: i64,
    done: bool,
    app: AppHandle,
    shared: State<'_, Shared>,
) -> AppResult<()> {
    store::set_done(&shared.db.lock(), id, done)?;
    changed(&app);
    Ok(())
}

#[tauri::command]
pub fn notes_delete(id: i64, app: AppHandle, shared: State<'_, Shared>) -> AppResult<()> {
    store::delete(&shared.db.lock(), id)?;
    changed(&app);
    Ok(())
}

#[tauri::command]
pub fn timer_get(shared: State<'_, Shared>) -> AppResult<TimerState> {
    Ok(*shared.timer())
}

/// Starts (or restarts) the timer.
#[tauri::command]
pub fn timer_start(
    duration_ms: u64,
    app: AppHandle,
    shared: State<'_, Shared>,
) -> AppResult<TimerState> {
    let state = TimerState::start(duration_ms, now_ms());
    *shared.timer() = state;
    *shared.ringer() = None;
    emit(&app, "notes://timer", state);
    Ok(state)
}

/// `pause`, `resume` or `cancel`.
#[tauri::command]
pub fn timer_control(
    action: String,
    app: AppHandle,
    shared: State<'_, Shared>,
) -> AppResult<TimerState> {
    let now = now_ms();
    let state = {
        let mut timer = shared.timer();
        *timer = match action.as_str() {
            "pause" => timer.pause(now),
            "resume" => timer.resume(now),
            "cancel" => TimerState::default(),
            other => {
                return Err(crate::error::AppError::InvalidArgument(format!(
                    "неизвестное действие таймера {other}"
                )))
            }
        };
        *timer
    };
    emit(&app, "notes://timer", state);
    Ok(state)
}

/// Stops the "time's up" sound (the user reacted to the alert).
#[tauri::command]
pub fn timer_silence(shared: State<'_, Shared>) -> AppResult<()> {
    *shared.ringer() = None;
    Ok(())
}
