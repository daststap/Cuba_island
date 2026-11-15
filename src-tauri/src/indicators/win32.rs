//! Registry and keyboard access for the indicators.

use std::time::Duration;

use windows::core::{HSTRING, PCWSTR, PWSTR};
use windows::Win32::Foundation::{CloseHandle, ERROR_NO_MORE_ITEMS, ERROR_SUCCESS, HANDLE};
use windows::Win32::System::Registry::{
    RegCloseKey, RegEnumKeyExW, RegGetValueW, RegNotifyChangeKeyValue, RegOpenKeyExW, HKEY,
    HKEY_CURRENT_USER, KEY_NOTIFY, KEY_READ, REG_NOTIFY_CHANGE_LAST_SET, REG_NOTIFY_CHANGE_NAME,
    RRF_RT_REG_QWORD,
};
use windows::Win32::System::Threading::{CreateEventW, WaitForMultipleObjects};
use windows::Win32::UI::Input::KeyboardAndMouse::{GetKeyState, VK_CAPITAL};

use super::{app_name, in_use, tidy};

const CONSENT_STORE: &str =
    r"Software\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\ConsentStore";
const NON_PACKAGED: &str = "NonPackaged";
const CAPABILITIES: [&str; 2] = ["microphone", "webcam"];
/// Re-read anyway now and then, and retry if the keys do not exist yet.
const FALLBACK: Duration = Duration::from_secs(30);
/// Registry key names are at most 255 characters.
const MAX_KEY_NAME: usize = 256;

/// An open registry key, closed on drop.
struct Key(HKEY);

impl Key {
    fn open(path: &str) -> Option<Self> {
        let mut key = HKEY::default();
        // SAFETY: `path` outlives the call (HSTRING is NUL-terminated); `key` is a
        // valid out-pointer.
        let status = unsafe {
            RegOpenKeyExW(
                HKEY_CURRENT_USER,
                &HSTRING::from(path),
                None,
                KEY_READ | KEY_NOTIFY,
                &mut key,
            )
        };
        (status == ERROR_SUCCESS).then_some(Self(key))
    }

    fn subkeys(&self) -> Vec<String> {
        let mut names = Vec::new();
        let mut buf = [0u16; MAX_KEY_NAME];
        for index in 0.. {
            let mut len = buf.len() as u32;
            // SAFETY: `buf` is writable for `len` UTF-16 units; the key is open.
            let status = unsafe {
                RegEnumKeyExW(
                    self.0,
                    index,
                    Some(PWSTR(buf.as_mut_ptr())),
                    &mut len,
                    None,
                    None,
                    None,
                    None,
                )
            };
            if status == ERROR_NO_MORE_ITEMS || status != ERROR_SUCCESS {
                break;
            }
            names.push(String::from_utf16_lossy(&buf[..len as usize]));
        }
        names
    }

    fn qword(&self, subkey: &str, value: &str) -> Option<u64> {
        let mut data = 0u64;
        let mut size = std::mem::size_of::<u64>() as u32;
        // SAFETY: `data` is a writable u64 and `size` holds its size; the strings
        // outlive the call.
        let status = unsafe {
            RegGetValueW(
                self.0,
                &HSTRING::from(subkey),
                &HSTRING::from(value),
                RRF_RT_REG_QWORD,
                None,
                Some((&raw mut data).cast()),
                Some(&mut size),
            )
        };
        (status == ERROR_SUCCESS).then_some(data)
    }

    fn in_use(&self, subkey: &str) -> bool {
        in_use(
            self.qword(subkey, "LastUsedTimeStart"),
            self.qword(subkey, "LastUsedTimeStop"),
        )
    }
}

impl Drop for Key {
    fn drop(&mut self) {
        // SAFETY: the key was opened by `Key::open` and is closed exactly once.
        let _ = unsafe { RegCloseKey(self.0) };
    }
}

/// Names of the apps using `capability` (`microphone` or `webcam`) right now.
pub fn apps_using(capability: &str) -> Vec<String> {
    let path = format!(r"{CONSENT_STORE}\{capability}");
    let Some(store) = Key::open(&path) else {
        return Vec::new();
    };
    let mut names = Vec::new();
    for sub in store.subkeys() {
        if sub == NON_PACKAGED {
            if let Some(desktop) = Key::open(&format!(r"{path}\{NON_PACKAGED}")) {
                for app in desktop.subkeys() {
                    if desktop.in_use(&app) {
                        names.push(app_name(&app, false));
                    }
                }
            }
        } else if store.in_use(&sub) {
            names.push(app_name(&sub, true));
        }
    }
    tidy(names)
}

/// An auto-reset event, closed on drop.
struct Event(HANDLE);

impl Event {
    fn new() -> Option<Self> {
        // SAFETY: no security attributes or name; the handle is owned by `Event`.
        unsafe { CreateEventW(None, false, false, PCWSTR::null()) }
            .ok()
            .map(Self)
    }
}

impl Drop for Event {
    fn drop(&mut self) {
        // SAFETY: the handle came from CreateEventW and is closed exactly once.
        let _ = unsafe { CloseHandle(self.0) };
    }
}

/// Calls `on_change` whenever anything under the microphone or camera keys
/// changes (and every 30 s as a fallback). Never returns.
pub fn watch_consent_store(on_change: &mut dyn FnMut()) {
    loop {
        let keys: Vec<Key> = CAPABILITIES
            .iter()
            .filter_map(|c| Key::open(&format!(r"{CONSENT_STORE}\{c}")))
            .collect();
        let events: Vec<Event> = keys.iter().filter_map(|_| Event::new()).collect();
        if keys.is_empty() || events.len() != keys.len() {
            std::thread::sleep(FALLBACK);
            on_change();
            continue;
        }
        // Re-arm and wait until the keys change; reopen them if arming fails.
        loop {
            let armed = keys.iter().zip(&events).all(|(key, event)| {
                // SAFETY: the key and event stay open while the notification is armed
                // (both live until this loop ends); the thread keeps running, which
                // async registry notifications require.
                let status = unsafe {
                    RegNotifyChangeKeyValue(
                        key.0,
                        true,
                        REG_NOTIFY_CHANGE_NAME | REG_NOTIFY_CHANGE_LAST_SET,
                        Some(event.0),
                        true,
                    )
                };
                status == ERROR_SUCCESS
            });
            if !armed {
                log::warn!("failed to watch the microphone/camera registry keys");
                std::thread::sleep(FALLBACK);
                break;
            }
            let handles: Vec<HANDLE> = events.iter().map(|e| e.0).collect();
            // SAFETY: all handles are valid events owned by `events`.
            let _ = unsafe { WaitForMultipleObjects(&handles, false, FALLBACK.as_millis() as u32) };
            on_change();
        }
    }
}

/// Whether Caps Lock is on (the toggle bit of its key state).
pub fn caps_lock_on() -> bool {
    // SAFETY: plain query of the keyboard state; no pointers involved.
    let state = unsafe { GetKeyState(i32::from(VK_CAPITAL.0)) };
    state & 1 != 0
}
