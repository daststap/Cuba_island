//! Clipboard checks that need raw Win32: privacy formats and the clipboard owner.

use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{HANDLE, HGLOBAL};
use windows::Win32::System::DataExchange::{
    CloseClipboard, GetClipboardData, GetClipboardOwner, IsClipboardFormatAvailable, OpenClipboard,
    RegisterClipboardFormatW,
};
use windows::Win32::System::Memory::{GlobalLock, GlobalSize, GlobalUnlock};
use windows::Win32::System::Threading::GetCurrentProcessId;
use windows::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId;

fn format_id(name: PCWSTR) -> Option<u32> {
    // SAFETY: `name` is a static, NUL-terminated wide string.
    let id = unsafe { RegisterClipboardFormatW(name) };
    (id != 0).then_some(id)
}

fn has_format(id: u32) -> bool {
    // SAFETY: plain query by format id.
    unsafe { IsClipboardFormatAvailable(id) }.is_ok()
}

/// Opens the clipboard, retrying while another app still holds it.
fn open_clipboard() -> bool {
    for attempt in 0..OPEN_ATTEMPTS {
        // SAFETY: opening without an owner window; closed by the caller on success.
        if unsafe { OpenClipboard(None) }.is_ok() {
            return true;
        }
        if attempt + 1 < OPEN_ATTEMPTS {
            std::thread::sleep(OPEN_RETRY_DELAY);
        }
    }
    false
}

const OPEN_ATTEMPTS: u32 = 5;
const OPEN_RETRY_DELAY: std::time::Duration = std::time::Duration::from_millis(20);

/// Reads a DWORD-sized clipboard format, as used by `CanIncludeInClipboardHistory`.
fn read_dword_format(id: u32) -> Option<u32> {
    if !open_clipboard() {
        return None;
    }
    // SAFETY: the clipboard is open (and always closed below); the global memory is
    // locked only while reading 4 bytes from it, after checking its size.
    unsafe {
        let value = (|| {
            let handle: HANDLE = GetClipboardData(id).ok()?;
            let global = HGLOBAL(handle.0);
            if GlobalSize(global) < 4 {
                return None;
            }
            let ptr = GlobalLock(global).cast::<u32>();
            if ptr.is_null() {
                return None;
            }
            let value = ptr.read_unaligned();
            let _ = GlobalUnlock(global);
            Some(value)
        })();
        let _ = CloseClipboard();
        value
    }
}

/// Whether the current clipboard content must not be recorded (PLAN.md stage 3):
/// password managers mark secrets with these formats.
pub fn is_private() -> bool {
    let exclude = [
        w!("ExcludeClipboardContentFromMonitorProcessing"),
        w!("Clipboard Viewer Ignore"),
    ];
    if exclude.into_iter().filter_map(format_id).any(has_format) {
        return true;
    }
    match format_id(w!("CanIncludeInClipboardHistory")).filter(|id| has_format(*id)) {
        None => false,
        // Fail closed: if the flag exists but cannot be read, treat the clip as private.
        Some(id) => read_dword_format(id).is_none_or(|allowed| allowed == 0),
    }
}

/// Whether the clipboard was last written by this process (our own copy/paste).
pub fn written_by_us() -> bool {
    // SAFETY: plain queries; a null owner yields pid 0, which never matches ours.
    unsafe {
        let Ok(owner) = GetClipboardOwner() else {
            return false;
        };
        let mut pid = 0;
        GetWindowThreadProcessId(owner, Some(&mut pid));
        pid != 0 && pid == GetCurrentProcessId()
    }
}
