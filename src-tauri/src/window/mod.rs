//! The island window: placement, click-through and full-screen hiding (PLAN.md §4.1–4.4).

mod geometry;
#[cfg(windows)]
mod input;
#[cfg(windows)]
mod tracker;
#[cfg(windows)]
mod win32;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use tauri::{AppHandle, Manager, PhysicalPosition, PhysicalSize, State, WebviewWindow};

use crate::error::{AppError, AppResult};
use geometry::{IslandRect, PhysRect, Placement};

pub const WINDOW_LABEL: &str = "island";
/// Fixed logical window size; only the island inside it changes shape (PLAN.md §7).
const WINDOW_SIZE: (f64, f64) = (800.0, 560.0);

/// State shared between commands and the background threads.
#[derive(Default)]
pub struct IslandShared {
    rect: Mutex<IslandRect>,
    placement: Mutex<Option<Placement>>,
    hidden: AtomicBool,
    /// Settings: target monitor name (`None` = primary) and full-screen hiding.
    monitor: Mutex<Option<String>>,
    hide_in_fullscreen: AtomicBool,
    /// The island accepts keyboard focus (search field, hotkeys).
    input_mode: AtomicBool,
    /// Where focus goes back to when input mode ends; also the paste target.
    #[cfg(windows)]
    previous_foreground: Mutex<Option<win32::Hwnd>>,
}

impl IslandShared {
    #[cfg_attr(not(windows), allow(dead_code))]
    fn rect(&self) -> IslandRect {
        *self.rect.lock().unwrap_or_else(PoisonError::into_inner)
    }

    #[cfg_attr(not(windows), allow(dead_code))]
    fn placement(&self) -> Option<Placement> {
        *self
            .placement
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }
}

type Shared = Arc<IslandShared>;

/// Positions the window, installs click-through and starts the background watchers.
pub fn setup(app: &AppHandle, monitor: Option<String>, hide_in_fullscreen: bool) -> AppResult<()> {
    let window = app
        .get_webview_window(WINDOW_LABEL)
        .ok_or_else(|| AppError::Internal(format!("window `{WINDOW_LABEL}` is missing")))?;
    let shared: Shared = Arc::new(IslandShared {
        monitor: Mutex::new(monitor),
        hide_in_fullscreen: AtomicBool::new(hide_in_fullscreen),
        ..IslandShared::default()
    });
    app.manage(shared.clone());

    reposition(&window, &shared)?;

    #[cfg(windows)]
    let hwnd = {
        let hwnd = win32::Hwnd::of(&window)?;
        let (tx, rx) = std::sync::mpsc::channel();
        win32::install(hwnd, tx)?;
        tracker::spawn_reposition_worker(window.clone(), shared.clone(), rx);
        hwnd
    };

    // Created hidden with `focus: false`, so tao shows it with SW_SHOWNOACTIVATE.
    window.show()?;

    // Start watching only after the first show: otherwise a full-screen app found at
    // startup hides the window, and this show() would bring it back over the game.
    #[cfg(windows)]
    {
        tracker::spawn_cursor_tracker(app.clone(), hwnd, shared.clone());
        tracker::spawn_fullscreen_watcher(app.clone(), hwnd, shared.clone());
    }
    Ok(())
}

/// Reports the island's current rectangle (logical px, relative to the window).
#[tauri::command]
pub fn set_island_rect(x: f64, y: f64, w: f64, h: f64, shared: State<'_, Shared>) -> AppResult<()> {
    let rect = IslandRect::new(x, y, w, h)?;
    *shared.rect.lock().unwrap_or_else(PoisonError::into_inner) = rect;
    Ok(())
}

/// Turns input mode on (the island takes keyboard focus) or off (focus returns
/// to the window the user was in, if the island still has it).
#[tauri::command]
pub fn set_input_mode(app: AppHandle, enabled: bool) -> AppResult<()> {
    if enabled {
        enter_input_mode(&app)
    } else {
        exit_input_mode(&app)
    }
}

fn island(app: &AppHandle) -> AppResult<(WebviewWindow, Shared)> {
    let window = app
        .get_webview_window(WINDOW_LABEL)
        .ok_or_else(|| AppError::Internal(format!("window `{WINDOW_LABEL}` is missing")))?;
    Ok((window, app.state::<Shared>().inner().clone()))
}

/// Remembers the user's window and gives the island keyboard focus.
/// Does nothing while the island is hidden by a full-screen app.
pub fn enter_input_mode(app: &AppHandle) -> AppResult<()> {
    let (window, shared) = island(app)?;
    if shared.hidden.load(Ordering::SeqCst) {
        return Ok(());
    }
    #[cfg(windows)]
    {
        let own = win32::Hwnd::of(&window)?;
        // A repeated call while the island already has focus keeps the original target.
        if let Some(fg) = input::foreground().filter(|fg| *fg != own) {
            *shared
                .previous_foreground
                .lock()
                .unwrap_or_else(PoisonError::into_inner) = Some(fg);
        }
    }
    shared.input_mode.store(true, Ordering::SeqCst);
    window.set_focusable(true)?;
    window.set_focus()?;
    #[cfg(windows)]
    log::debug!(
        "input mode on; island has focus: {}",
        input::foreground() == Some(win32::Hwnd::of(&window)?)
    );
    Ok(())
}

/// Makes the island unfocusable again and returns focus to the user's window —
/// unless the user already switched elsewhere themselves.
pub fn exit_input_mode(app: &AppHandle) -> AppResult<()> {
    leave_input_mode(app, true)
}

/// Leaves input mode without returning focus, and lets any process take the
/// foreground: used right before launching an app or opening the browser, so the
/// new window comes to the front instead of the one the user was in.
pub fn hand_over_focus(app: &AppHandle) -> AppResult<()> {
    #[cfg(windows)]
    input::allow_any_foreground();
    leave_input_mode(app, false)
}

fn leave_input_mode(app: &AppHandle, restore: bool) -> AppResult<()> {
    let (window, shared) = island(app)?;
    if !shared.input_mode.swap(false, Ordering::SeqCst) {
        return Ok(());
    }
    #[cfg(windows)]
    let island_has_focus = input::foreground() == Some(win32::Hwnd::of(&window)?);
    window.set_focusable(false)?;
    #[cfg(windows)]
    if restore && island_has_focus {
        let previous = *shared
            .previous_foreground
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if let Some(previous) = previous {
            if !input::activate(previous) {
                log::warn!("could not return focus to the previous window");
            }
        }
    }
    Ok(())
}

/// Delay between focusing the target window and pressing Ctrl+V (PLAN.md stage 3).
const PASTE_DELAY: Duration = Duration::from_millis(50);

/// Leaves input mode (returning focus to the user's window) and presses Ctrl+V there.
/// Blocks for `PASTE_DELAY`; call it off the main thread.
pub fn paste_into_previous(app: &AppHandle) -> AppResult<()> {
    exit_input_mode(app)?;
    std::thread::sleep(PASTE_DELAY);
    #[cfg(windows)]
    if !input::send_paste() {
        return Err(AppError::Internal("не удалось отправить Ctrl+V".into()));
    }
    Ok(())
}

/// Gives `window` (another window of the app) the keyboard focus, even when
/// Windows' foreground rules would refuse a plain request.
pub fn bring_to_front(window: &tauri::WebviewWindow) -> AppResult<()> {
    window.set_focus()?;
    #[cfg(windows)]
    {
        let hwnd = win32::Hwnd::of(window)?;
        if input::foreground() != Some(hwnd) && !input::activate(hwnd) {
            log::warn!("could not bring {} to the foreground", window.label());
        }
    }
    Ok(())
}

/// Whether the island is currently hidden because a full-screen app is in front.
pub fn is_hidden(app: &AppHandle) -> bool {
    app.try_state::<Shared>()
        .is_some_and(|s| s.hidden.load(Ordering::SeqCst))
}

/// A monitor the island can be placed on. Mirrored in `src/ipc/commands.ts`.
#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MonitorInfo {
    /// Windows device name, e.g. `\\.\DISPLAY1`; stored in settings.
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub primary: bool,
}

pub fn monitors(app: &AppHandle) -> AppResult<Vec<MonitorInfo>> {
    let (window, _) = island(app)?;
    let primary = window.primary_monitor()?.and_then(|m| m.name().cloned());
    Ok(window
        .available_monitors()?
        .into_iter()
        .filter_map(|m| {
            let name = m.name()?.clone();
            Some(MonitorInfo {
                primary: primary.as_ref() == Some(&name),
                width: m.size().width,
                height: m.size().height,
                name,
            })
        })
        .collect())
}

/// Applies display settings: which monitor (`None` = primary) and whether to hide
/// while a full-screen app is in front.
pub fn apply_display(
    app: &AppHandle,
    monitor: Option<String>,
    hide_in_fullscreen: bool,
) -> AppResult<()> {
    let (window, shared) = island(app)?;
    shared
        .hide_in_fullscreen
        .store(hide_in_fullscreen, Ordering::SeqCst);
    let changed = {
        let mut current = shared
            .monitor
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let changed = *current != monitor;
        *current = monitor;
        changed
    };
    if changed {
        reposition(&window, &shared)?;
    }
    Ok(())
}

/// Places the window at the top centre of the target monitor (PLAN.md §4.3).
fn reposition(window: &WebviewWindow, shared: &Shared) -> AppResult<()> {
    let wanted = shared
        .monitor
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clone();
    // The chosen monitor if it is connected, otherwise the primary one.
    let chosen = wanted.and_then(|name| {
        window
            .available_monitors()
            .ok()?
            .into_iter()
            .find(|m| m.name() == Some(&name))
    });
    let monitor = match chosen {
        Some(monitor) => monitor,
        None => window
            .primary_monitor()?
            .ok_or_else(|| AppError::Internal("no primary monitor".into()))?,
    };
    let scale = monitor.scale_factor();
    let rect = PhysRect {
        x: monitor.position().x,
        y: monitor.position().y,
        w: i32::try_from(monitor.size().width).unwrap_or(i32::MAX),
        h: i32::try_from(monitor.size().height).unwrap_or(i32::MAX),
    };
    let size = geometry::physical_size(WINDOW_SIZE, scale);
    let origin = geometry::window_origin(rect, size.0);

    window.set_position(PhysicalPosition::new(origin.0, origin.1))?;
    window.set_size(PhysicalSize::new(size.0, size.1))?;

    *shared
        .placement
        .lock()
        .unwrap_or_else(PoisonError::into_inner) = Some(Placement {
        window_origin: origin,
        scale,
        monitor: rect,
    });
    log::info!("island placed at {origin:?}, size {size:?}, scale {scale}, monitor {rect:?}");
    Ok(())
}
