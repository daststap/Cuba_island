//! Win32 focus and keyboard helpers for input mode and pasting (PLAN.md §4.1, stage 3).

use std::mem::size_of;

use windows::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP,
    VIRTUAL_KEY, VK_CONTROL, VK_V,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AllowSetForegroundWindow, GetForegroundWindow, GetWindowThreadProcessId, IsWindow,
    SetForegroundWindow, ASFW_ANY,
};

use super::win32::Hwnd;

/// The window the user is working in right now.
pub fn foreground() -> Option<Hwnd> {
    // SAFETY: takes no arguments; a null handle means "no foreground window".
    let hwnd = unsafe { GetForegroundWindow() };
    (!hwnd.is_invalid()).then(|| Hwnd::from_raw(hwnd))
}

/// Brings `hwnd` to the foreground. Falls back to attaching input queues, which
/// Windows requires when the caller is not the foreground thread (PLAN.md §7).
pub fn activate(hwnd: Hwnd) -> bool {
    let raw = hwnd.raw();
    // SAFETY: IsWindow / SetForegroundWindow accept any handle; stale ones just fail.
    unsafe {
        if !IsWindow(Some(raw)).as_bool() {
            return false;
        }
        if SetForegroundWindow(raw).as_bool() {
            return true;
        }
    }
    // SAFETY: attaching our thread's input to the foreground thread's for the duration
    // of the SetForegroundWindow call, then detaching; all calls tolerate bad ids.
    unsafe {
        let ours = GetCurrentThreadId();
        let theirs = GetWindowThreadProcessId(GetForegroundWindow(), None);
        let attached =
            theirs != 0 && theirs != ours && AttachThreadInput(ours, theirs, true).as_bool();
        let ok = SetForegroundWindow(raw).as_bool();
        if attached {
            let _ = AttachThreadInput(ours, theirs, false);
        }
        ok
    }
}

/// Lets whatever process opens a window next bring it to the foreground.
/// Only works while we hold the foreground (input mode), which is when it's called.
pub fn allow_any_foreground() {
    // SAFETY: plain call; failure only means Windows keeps its usual focus rules.
    if let Err(err) = unsafe { AllowSetForegroundWindow(ASFW_ANY) } {
        log::debug!("AllowSetForegroundWindow failed: {err}");
    }
}

/// Presses Ctrl+V in whatever window has the keyboard focus.
pub fn send_paste() -> bool {
    let key = |vk: VIRTUAL_KEY, up: bool| INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                dwFlags: if up {
                    KEYEVENTF_KEYUP
                } else {
                    KEYBD_EVENT_FLAGS(0)
                },
                ..Default::default()
            },
        },
    };
    let inputs = [
        key(VK_CONTROL, false),
        key(VK_V, false),
        key(VK_V, true),
        key(VK_CONTROL, true),
    ];
    // SAFETY: `inputs` is a valid slice of fully initialised INPUT structs and the
    // size argument matches the struct size.
    let sent = unsafe { SendInput(&inputs, size_of::<INPUT>() as i32) };
    sent as usize == inputs.len()
}
