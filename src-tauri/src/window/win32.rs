//! Safe wrappers over the raw Win32 calls the island window needs.
//!
//! Why not `WebviewWindow::set_ignore_cursor_events`: tao rewrites the whole
//! `GWL_EXSTYLE` and calls `ShowWindow(SW_SHOW)` on every flag change, which would
//! drop `WS_EX_TOOLWINDOW` and can activate the window each time the cursor crosses
//! the island. Instead, click-through is toggled here, and a window subclass
//! re-applies our extended styles whenever anyone (including tao) changes them.

use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::OnceLock;

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::UI::Shell::{
    DefSubclassProc, SHQueryUserNotificationState, SetWindowSubclass, QUNS_BUSY,
    QUNS_PRESENTATION_MODE, QUNS_RUNNING_D3D_FULL_SCREEN,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetCursorPos, GetWindowLongPtrW, SetWindowLongPtrW, ShowWindow, GWL_EXSTYLE, MA_NOACTIVATE,
    STYLESTRUCT, SW_HIDE, SW_SHOWNOACTIVATE, WINDOW_EX_STYLE, WM_DISPLAYCHANGE, WM_DPICHANGED,
    WM_MOUSEACTIVATE, WM_STYLECHANGING, WS_EX_APPWINDOW, WS_EX_LAYERED, WS_EX_NOACTIVATE,
    WS_EX_TOOLWINDOW, WS_EX_TRANSPARENT,
};

use crate::error::{AppError, AppResult};

/// Whether the window currently lets mouse input fall through.
static CLICK_THROUGH: AtomicBool = AtomicBool::new(true);
/// Notified on `WM_DISPLAYCHANGE` / `WM_DPICHANGED` so the window gets repositioned.
static DISPLAY_CHANGED: OnceLock<Sender<()>> = OnceLock::new();

const SUBCLASS_ID: usize = 0xE5CA;

/// A window handle that can be moved between threads (Win32 handles are process-global).
#[derive(Clone, Copy, Debug)]
pub struct Hwnd(isize);

impl Hwnd {
    pub fn of(window: &tauri::WebviewWindow) -> AppResult<Self> {
        Ok(Self(window.hwnd()?.0 as isize))
    }

    fn raw(self) -> HWND {
        HWND(self.0 as *mut c_void)
    }
}

/// Subclasses the window and applies the initial extended styles.
/// Must be called on the thread that owns the window (the main thread).
pub fn install(hwnd: Hwnd, on_display_change: Sender<()>) -> AppResult<()> {
    let _ = DISPLAY_CHANGED.set(on_display_change);
    // SAFETY: `hwnd` is a live window created by this process on the current thread,
    // and `subclass_proc` has exactly the SUBCLASSPROC signature.
    let installed = unsafe { SetWindowSubclass(hwnd.raw(), Some(subclass_proc), SUBCLASS_ID, 0) };
    if !installed.as_bool() {
        return Err(AppError::Internal("SetWindowSubclass failed".into()));
    }
    apply_ex_style(hwnd);
    Ok(())
}

/// Turns mouse click-through on or off. Safe to call from any thread.
pub fn set_click_through(hwnd: Hwnd, enabled: bool) {
    CLICK_THROUGH.store(enabled, Ordering::SeqCst);
    apply_ex_style(hwnd);
}

/// Shows or hides the window without ever activating it.
pub fn set_visible(hwnd: Hwnd, visible: bool) {
    let cmd = if visible { SW_SHOWNOACTIVATE } else { SW_HIDE };
    // SAFETY: ShowWindow accepts any HWND; a stale handle makes it a no-op.
    let _ = unsafe { ShowWindow(hwnd.raw(), cmd) };
}

/// Cursor position in physical screen pixels.
pub fn cursor_pos() -> Option<(i32, i32)> {
    let mut point = POINT::default();
    // SAFETY: `point` is a valid, writable POINT for the duration of the call.
    unsafe { GetCursorPos(&mut point) }.ok()?;
    Some((point.x, point.y))
}

/// Whether a full-screen app, D3D game or presentation is in front (PLAN.md §4.4).
pub fn fullscreen_app_active() -> bool {
    // SAFETY: takes no pointers; the result is returned by value.
    match unsafe { SHQueryUserNotificationState() } {
        Ok(state) => {
            state == QUNS_BUSY
                || state == QUNS_RUNNING_D3D_FULL_SCREEN
                || state == QUNS_PRESENTATION_MODE
        }
        Err(err) => {
            log::debug!("SHQueryUserNotificationState failed: {err}");
            false
        }
    }
}

/// The extended style we want, given the current one.
fn enforce_ex_style(current: u32, click_through: bool) -> u32 {
    let mut style = WINDOW_EX_STYLE(current);
    // Tool window: not in Alt+Tab. Never an app window (that forces a taskbar button).
    style |= WS_EX_TOOLWINDOW;
    style &= !WS_EX_APPWINDOW;
    if click_through {
        style |= WS_EX_TRANSPARENT | WS_EX_LAYERED;
    } else {
        style &= !(WS_EX_TRANSPARENT | WS_EX_LAYERED);
    }
    style.0
}

fn apply_ex_style(hwnd: Hwnd) {
    // SAFETY: Get/SetWindowLongPtrW accept any HWND; a stale handle makes them fail
    // harmlessly. The style is a 32-bit set stored in a pointer-sized slot, so the
    // casts are lossless.
    unsafe {
        let current = GetWindowLongPtrW(hwnd.raw(), GWL_EXSTYLE) as u32;
        let wanted = enforce_ex_style(current, CLICK_THROUGH.load(Ordering::SeqCst));
        if wanted != current {
            SetWindowLongPtrW(hwnd.raw(), GWL_EXSTYLE, wanted as isize);
        }
    }
}

unsafe extern "system" fn subclass_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _id: usize,
    _ref_data: usize,
) -> LRESULT {
    match msg {
        WM_STYLECHANGING if wparam.0 as i32 == GWL_EXSTYLE.0 => {
            // SAFETY: for WM_STYLECHANGING, lParam points to a STYLESTRUCT that the
            // window procedure may modify before the change is applied.
            let change = unsafe { &mut *(lparam.0 as *mut STYLESTRUCT) };
            change.styleNew =
                enforce_ex_style(change.styleNew, CLICK_THROUGH.load(Ordering::SeqCst));
        }
        WM_MOUSEACTIVATE => {
            // SAFETY: reading a window long of our own window on its own thread.
            let ex = unsafe { GetWindowLongPtrW(hwnd, GWL_EXSTYLE) } as u32;
            if ex & WS_EX_NOACTIVATE.0 != 0 {
                return LRESULT(MA_NOACTIVATE as isize);
            }
        }
        WM_DISPLAYCHANGE | WM_DPICHANGED => {
            // SAFETY: forwarding the unmodified message to the next window procedure.
            let result = unsafe { DefSubclassProc(hwnd, msg, wparam, lparam) };
            if let Some(tx) = DISPLAY_CHANGED.get() {
                let _ = tx.send(());
            }
            return result;
        }
        _ => {}
    }
    // SAFETY: forwarding the (possibly adjusted) message to the next window procedure.
    unsafe { DefSubclassProc(hwnd, msg, wparam, lparam) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enforces_tool_window_and_click_through() {
        let style = enforce_ex_style(WS_EX_APPWINDOW.0 | WS_EX_NOACTIVATE.0, true);
        assert_ne!(style & WS_EX_TOOLWINDOW.0, 0);
        assert_eq!(style & WS_EX_APPWINDOW.0, 0);
        assert_ne!(style & WS_EX_TRANSPARENT.0, 0);
        assert_ne!(
            style & WS_EX_NOACTIVATE.0,
            0,
            "NOACTIVATE is owned by tao and kept"
        );
    }

    #[test]
    fn removes_click_through_when_disabled() {
        let start = WS_EX_TRANSPARENT.0 | WS_EX_LAYERED.0 | WS_EX_TOOLWINDOW.0;
        let style = enforce_ex_style(start, false);
        assert_eq!(style & (WS_EX_TRANSPARENT.0 | WS_EX_LAYERED.0), 0);
        assert_ne!(style & WS_EX_TOOLWINDOW.0, 0);
    }
}
