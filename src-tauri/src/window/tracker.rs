//! Background threads: cursor tracking, full-screen detection, repositioning.

use std::sync::atomic::Ordering;
use std::sync::mpsc::Receiver;
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter, WebviewWindow};

use super::geometry::{in_edge_zone, to_logical};
use super::win32::{self, Hwnd};
use super::{reposition, Shared};
use crate::thread::spawn_supervised;

const CURSOR_POLL: Duration = Duration::from_millis(16);
const FULLSCREEN_POLL: Duration = Duration::from_secs(1);
/// Display changes arrive in bursts; wait for them to settle before repositioning.
const REPOSITION_DEBOUNCE: Duration = Duration::from_millis(150);
/// Extra logical px around the island that still counts as hovering.
const HOVER_MARGIN: f64 = 4.0;
/// Half-width (logical px) of the top-edge zone that expands the island.
const EDGE_HALF_WIDTH: f64 = 150.0;

#[derive(Clone, Serialize)]
struct HoverPayload {
    inside: bool,
}

#[derive(Clone, Serialize)]
struct VisibilityPayload {
    visible: bool,
}

fn emit<P: Serialize + Clone>(app: &AppHandle, event: &str, payload: P) {
    if let Err(err) = app.emit(event, payload) {
        log::warn!("failed to emit {event}: {err}");
    }
}

/// Polls the cursor every 16 ms and toggles click-through around the island (PLAN.md §4.2).
pub fn spawn_cursor_tracker(app: AppHandle, hwnd: Hwnd, shared: Shared) {
    spawn_supervised("island-cursor", move || {
        let mut inside = false;
        let mut at_edge = false;
        win32::set_click_through(hwnd, true);
        loop {
            std::thread::sleep(CURSOR_POLL);
            let (Some(cursor), Some(placement)) = (win32::cursor_pos(), shared.placement()) else {
                continue;
            };
            let hidden = shared.hidden.load(Ordering::SeqCst);

            let logical = to_logical(cursor, placement.window_origin, placement.scale);
            let now_inside = !hidden && shared.rect().contains(logical, HOVER_MARGIN);
            if now_inside != inside {
                inside = now_inside;
                win32::set_click_through(hwnd, !inside);
                emit(&app, "island://hover", HoverPayload { inside });
            }

            let now_at_edge = !hidden
                && in_edge_zone(cursor, placement.monitor, placement.scale, EDGE_HALF_WIDTH);
            if now_at_edge && !at_edge {
                emit(&app, "island://edge-push", ());
            }
            at_edge = now_at_edge;
        }
    });
}

/// Hides the island while a full-screen app is in front (PLAN.md §4.4).
pub fn spawn_fullscreen_watcher(app: AppHandle, hwnd: Hwnd, shared: Shared) {
    spawn_supervised("island-fullscreen", move || {
        let mut hidden = shared.hidden.load(Ordering::SeqCst);
        loop {
            let busy =
                shared.hide_in_fullscreen.load(Ordering::SeqCst) && win32::fullscreen_app_active();
            if busy != hidden {
                hidden = busy;
                shared.hidden.store(hidden, Ordering::SeqCst);
                win32::set_visible(hwnd, !hidden);
                log::info!(
                    "full-screen app {}",
                    if hidden { "detected" } else { "gone" }
                );
                emit(
                    &app,
                    "island://visibility",
                    VisibilityPayload { visible: !hidden },
                );
            }
            std::thread::sleep(FULLSCREEN_POLL);
        }
    });
}

/// Repositions the window after `WM_DISPLAYCHANGE` / `WM_DPICHANGED` (PLAN.md §4.3).
pub fn spawn_reposition_worker(window: WebviewWindow, shared: Shared, rx: Receiver<()>) {
    spawn_supervised("island-reposition", move || {
        while rx.recv().is_ok() {
            std::thread::sleep(REPOSITION_DEBOUNCE);
            while rx.try_recv().is_ok() {}
            if let Err(err) = reposition(&window, &shared) {
                log::error!("failed to reposition island: {err}");
            }
        }
    });
}
