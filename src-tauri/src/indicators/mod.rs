//! Microphone / camera in use (iPhone-style privacy dots) and Caps Lock
//! (PLAN.md stage 6).
//!
//! Windows records every app's use of the microphone and camera under
//! `HKCU\…\CapabilityAccessManager\ConsentStore\{microphone,webcam}`: an app is
//! using the device while its `LastUsedTimeStop` is 0. The keys are watched with
//! `RegNotifyChangeKeyValue`, so nothing is polled. Caps Lock has no change
//! notification without a keyboard hook, so its toggle bit is read every 100 ms.

#[cfg(windows)]
mod win32;

use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::error::AppResult;
use crate::thread::spawn_supervised;

const CAPS_POLL: Duration = Duration::from_millis(100);
/// Several values change at once when an app grabs a device; wait for them all.
const SETTLE: Duration = Duration::from_millis(150);

/// Apps using the microphone and the camera right now. Mirrored in `src/ipc/commands.ts`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Sensors {
    pub microphone: Vec<String>,
    pub camera: Vec<String>,
}

/// A device is in use while it has a start time and no stop time yet.
#[cfg_attr(not(windows), allow(dead_code))]
fn in_use(start: Option<u64>, stop: Option<u64>) -> bool {
    start.unwrap_or(0) != 0 && stop.unwrap_or(0) == 0
}

/// Readable name for a consent-store key:
/// `C:#Program Files#Discord#Discord.exe` → `Discord`,
/// `Microsoft.WindowsCamera_8wekyb3d8bbwe` → `WindowsCamera`.
#[cfg_attr(not(windows), allow(dead_code))]
fn app_name(key: &str, packaged: bool) -> String {
    if packaged {
        let family = key.split('_').next().unwrap_or(key);
        family.rsplit('.').next().unwrap_or(family).to_owned()
    } else {
        let file = key.rsplit('#').next().unwrap_or(key);
        let stem = file.rsplit_once('.').map_or(file, |(stem, ext)| {
            if ext.eq_ignore_ascii_case("exe") {
                stem
            } else {
                file
            }
        });
        stem.to_owned()
    }
}

/// Sorted, without duplicates (one app can have several keys, one per version).
#[cfg_attr(not(windows), allow(dead_code))]
fn tidy(mut names: Vec<String>) -> Vec<String> {
    names.sort_by_key(|n| n.to_lowercase());
    names.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
    names
}

#[cfg(windows)]
fn read_sensors() -> Sensors {
    Sensors {
        microphone: win32::apps_using("microphone"),
        camera: win32::apps_using("webcam"),
    }
}

#[cfg(not(windows))]
fn read_sensors() -> Sensors {
    Sensors::default()
}

#[cfg(windows)]
fn caps_lock_on() -> bool {
    win32::caps_lock_on()
}

#[cfg(not(windows))]
fn caps_lock_on() -> bool {
    false
}

/// Current state for a freshly loaded page. Mirrored in `src/ipc/commands.ts`.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Indicators {
    pub sensors: Sensors,
    pub caps_lock: bool,
}

#[tauri::command]
pub fn indicators_get() -> AppResult<Indicators> {
    Ok(Indicators {
        sensors: read_sensors(),
        caps_lock: caps_lock_on(),
    })
}

fn emit<P: Serialize + Clone>(app: &AppHandle, event: &str, payload: P) {
    if let Err(err) = app.emit(event, payload) {
        log::warn!("failed to emit {event}: {err}");
    }
}

/// Starts the watchers: `indicators://sensors` and `indicators://caps-lock`.
pub fn start(app: &AppHandle) {
    let sensors_app = app.clone();
    spawn_supervised("indicators-sensors", move || {
        let mut last = read_sensors();
        let mut on_change = || {
            std::thread::sleep(SETTLE);
            let now = read_sensors();
            if now != last {
                log::info!(
                    "sensors in use: microphone {:?}, camera {:?}",
                    now.microphone,
                    now.camera
                );
                last = now.clone();
                emit(&sensors_app, "indicators://sensors", now);
            }
        };
        #[cfg(windows)]
        win32::watch_consent_store(&mut on_change);
        #[cfg(not(windows))]
        {
            let _ = &mut on_change;
        }
    });

    let caps_app = app.clone();
    spawn_supervised("indicators-caps", move || {
        let mut last = caps_lock_on();
        loop {
            std::thread::sleep(CAPS_POLL);
            let now = caps_lock_on();
            if now != last {
                last = now;
                emit(&caps_app, "indicators://caps-lock", now);
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn in_use_until_stopped() {
        assert!(in_use(Some(134_000), Some(0)));
        assert!(in_use(Some(134_000), None));
        assert!(!in_use(Some(134_000), Some(134_500)));
        assert!(!in_use(None, None));
        assert!(!in_use(Some(0), Some(0)));
    }

    #[test]
    fn names_desktop_apps_by_exe() {
        assert_eq!(
            app_name(
                r"C:#Users#me#AppData#Local#Discord#app-1.0.9#Discord.exe",
                false
            ),
            "Discord"
        );
        assert_eq!(app_name("C:#Tools#obs64.EXE", false), "obs64");
        assert_eq!(app_name("weird", false), "weird");
    }

    #[test]
    fn names_store_apps_by_package() {
        assert_eq!(
            app_name("Microsoft.WindowsCamera_8wekyb3d8bbwe", true),
            "WindowsCamera"
        );
        assert_eq!(app_name("Claude_pzs8sxrjxfjjc", true), "Claude");
    }

    #[test]
    fn tidies_duplicates() {
        let names = ["Discord", "browser", "discord"].map(String::from).to_vec();
        assert_eq!(tidy(names), ["browser", "Discord"]);
    }
}
