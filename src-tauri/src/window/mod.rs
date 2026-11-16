//! The island window: placement, click-through and full-screen hiding (PLAN.md §4.1–4.4).
//!
//! The island can sit on any edge of any monitor (dragged there by the user):
//! the window is placed in the monitor's work area at the chosen edge and offset,
//! and the page is told where along the edge the island's centre is.

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

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, State, WebviewWindow};

use crate::error::{AppError, AppResult};
pub use geometry::Edge;
use geometry::{IslandRect, PhysRect, Placement};

pub const WINDOW_LABEL: &str = "island";

/// Where the island sits, for the page. Mirrored in `src/ipc/commands.ts`.
#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IslandPlacement {
    pub edge: Edge,
    /// The island's centre along the edge, in logical px from the window's left
    /// (top/bottom) or top (left/right).
    pub anchor: f64,
    /// The island is being dragged: the page shows a floating pill centred in the window.
    pub dragging: bool,
}

/// Display settings the window follows.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Display {
    /// Monitor name (`None` = primary).
    pub monitor: Option<String>,
    pub edge: Edge,
    /// Position along the edge, 0–1.
    pub offset: f64,
}

/// State shared between commands and the background threads.
#[derive(Default)]
pub struct IslandShared {
    rect: Mutex<IslandRect>,
    placement: Mutex<Option<Placement>>,
    hidden: AtomicBool,
    /// Settings: monitor, edge and offset; full-screen hiding.
    display: Mutex<Display>,
    hide_in_fullscreen: AtomicBool,
    /// The user is dragging the island to another place.
    dragging: AtomicBool,
    /// What the page was last told.
    island_placement: Mutex<Option<IslandPlacement>>,
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
pub fn setup(app: &AppHandle, display: Display, hide_in_fullscreen: bool) -> AppResult<()> {
    let window = app
        .get_webview_window(WINDOW_LABEL)
        .ok_or_else(|| AppError::Internal(format!("window `{WINDOW_LABEL}` is missing")))?;
    let shared: Shared = Arc::new(IslandShared {
        display: Mutex::new(display),
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

/// Applies display settings: monitor, edge and offset, and whether to hide while
/// a full-screen app is in front.
pub fn apply_display(app: &AppHandle, display: Display, hide_in_fullscreen: bool) -> AppResult<()> {
    let (window, shared) = island(app)?;
    shared
        .hide_in_fullscreen
        .store(hide_in_fullscreen, Ordering::SeqCst);
    let changed = {
        let mut current = shared
            .display
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let changed = *current != display;
        *current = display;
        changed
    };
    if changed {
        reposition(&window, &shared)?;
    }
    Ok(())
}

fn tell_page(window: &WebviewWindow, shared: &Shared, placement: IslandPlacement) {
    *shared
        .island_placement
        .lock()
        .unwrap_or_else(PoisonError::into_inner) = Some(placement);
    if let Err(err) = window.emit("island://placement", placement) {
        log::warn!("failed to emit island://placement: {err}");
    }
}

/// Where the island sits now (for a page that just loaded).
#[tauri::command]
pub fn island_placement(shared: State<'_, Shared>) -> AppResult<Option<IslandPlacement>> {
    Ok(*shared
        .island_placement
        .lock()
        .unwrap_or_else(PoisonError::into_inner))
}

/// Starts moving the island with the cursor until the mouse button is released;
/// then it snaps to the nearest edge of the monitor under the cursor and the new
/// place is saved in settings.
#[tauri::command]
pub fn island_drag_start(app: AppHandle) -> AppResult<()> {
    let (window, shared) = island(&app)?;
    if shared.dragging.swap(true, Ordering::SeqCst) {
        return Ok(());
    }
    let edge = shared
        .display
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .edge;
    let size = window.outer_size()?;
    tell_page(
        &window,
        &shared,
        IslandPlacement {
            edge,
            anchor: 0.0,
            dragging: true,
        },
    );
    std::thread::spawn(move || {
        if let Err(err) = follow_cursor(&app, &window, (size.width, size.height)) {
            log::error!("island drag failed: {err}");
        }
        shared.dragging.store(false, Ordering::SeqCst);
        if let Err(err) = reposition(&window, &shared) {
            log::error!("failed to reposition island: {err}");
        }
    });
    Ok(())
}

/// Moves the window centred on the cursor while the button is held, then saves
/// the edge nearest to where it was released.
#[cfg(windows)]
fn follow_cursor(app: &AppHandle, window: &WebviewWindow, (w, h): (u32, u32)) -> AppResult<()> {
    let half = (
        i32::try_from(w / 2).unwrap_or(0),
        i32::try_from(h / 2).unwrap_or(0),
    );
    let mut last = None;
    while win32::primary_button_down() {
        if let Some(cursor) = win32::cursor_pos() {
            if last != Some(cursor) {
                last = Some(cursor);
                window.set_position(PhysicalPosition::new(cursor.0 - half.0, cursor.1 - half.1))?;
            }
        }
        std::thread::sleep(Duration::from_millis(16));
    }
    let Some(cursor) = win32::cursor_pos() else {
        return Ok(());
    };
    let monitor = app
        .monitor_from_point(f64::from(cursor.0), f64::from(cursor.1))?
        .or(window.primary_monitor()?)
        .ok_or_else(|| AppError::Internal("no monitor under the cursor".into()))?;
    let area = monitor.work_area();
    let area = PhysRect {
        x: area.position.x,
        y: area.position.y,
        w: i32::try_from(area.size.width).unwrap_or(i32::MAX),
        h: i32::try_from(area.size.height).unwrap_or(i32::MAX),
    };
    let (edge, offset) = geometry::nearest_edge(area, cursor);
    log::info!("island dropped on {edge:?} at {offset:.2}");
    crate::settings::set_placement(app, monitor.name().cloned(), edge, offset)
}

#[cfg(not(windows))]
fn follow_cursor(_app: &AppHandle, _window: &WebviewWindow, _size: (u32, u32)) -> AppResult<()> {
    Ok(())
}

/// Places the window on the chosen edge of the target monitor (PLAN.md §4.3).
fn reposition(window: &WebviewWindow, shared: &Shared) -> AppResult<()> {
    if shared.dragging.load(Ordering::SeqCst) {
        return Ok(());
    }
    let display = shared
        .display
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clone();
    // The chosen monitor if it is connected, otherwise the primary one.
    let chosen = display.monitor.clone().and_then(|name| {
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
    let work = monitor.work_area();
    let area = PhysRect {
        x: work.position.x,
        y: work.position.y,
        w: i32::try_from(work.size.width).unwrap_or(i32::MAX),
        h: i32::try_from(work.size.height).unwrap_or(i32::MAX),
    };
    let edge = display.edge;
    let size = geometry::physical_size(geometry::window_size(edge), scale);
    let (origin, centre) = geometry::place(area, edge, display.offset, size);

    window.set_size(PhysicalSize::new(size.0, size.1))?;
    window.set_position(PhysicalPosition::new(origin.0, origin.1))?;

    *shared
        .placement
        .lock()
        .unwrap_or_else(PoisonError::into_inner) = Some(Placement {
        window_origin: origin,
        scale,
        monitor: rect,
        edge,
        centre,
        flush: geometry::is_flush(rect, area, edge),
    });
    let along_origin = if edge.is_side() { origin.1 } else { origin.0 };
    tell_page(
        window,
        shared,
        IslandPlacement {
            edge,
            anchor: f64::from(centre - along_origin) / scale,
            dragging: false,
        },
    );
    log::info!(
        "island placed on {edge:?} at {origin:?}, size {size:?}, scale {scale}, monitor {rect:?}"
    );
    Ok(())
}
