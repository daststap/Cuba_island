//! The island window: placement, click-through and full-screen hiding (PLAN.md §4.1–4.4).

mod geometry;
#[cfg(windows)]
mod tracker;
#[cfg(windows)]
mod win32;

use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex, PoisonError};

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
pub fn setup(app: &AppHandle) -> AppResult<()> {
    let window = app
        .get_webview_window(WINDOW_LABEL)
        .ok_or_else(|| AppError::Internal(format!("window `{WINDOW_LABEL}` is missing")))?;
    let shared: Shared = Arc::default();
    app.manage(shared.clone());

    reposition(&window, &shared)?;

    #[cfg(windows)]
    {
        let hwnd = win32::Hwnd::of(&window)?;
        let (tx, rx) = std::sync::mpsc::channel();
        win32::install(hwnd, tx)?;
        tracker::spawn_reposition_worker(window.clone(), shared.clone(), rx);
        tracker::spawn_cursor_tracker(app.clone(), hwnd, shared.clone());
        tracker::spawn_fullscreen_watcher(app.clone(), hwnd, shared.clone());
    }

    // Created hidden with `focus: false`, so tao shows it with SW_SHOWNOACTIVATE.
    window.show()?;
    Ok(())
}

/// Reports the island's current rectangle (logical px, relative to the window).
#[tauri::command]
pub fn set_island_rect(x: f64, y: f64, w: f64, h: f64, shared: State<'_, Shared>) -> AppResult<()> {
    let rect = IslandRect::new(x, y, w, h)?;
    *shared.rect.lock().unwrap_or_else(PoisonError::into_inner) = rect;
    Ok(())
}

/// Places the window at the top centre of the target monitor (PLAN.md §4.3).
fn reposition(window: &WebviewWindow, shared: &Shared) -> AppResult<()> {
    // Monitor selection arrives with settings (stage 5); until then: the primary one.
    let monitor = window
        .primary_monitor()?
        .ok_or_else(|| AppError::Internal("no primary monitor".into()))?;
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
