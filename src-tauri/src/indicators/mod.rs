//! Microphone / camera in use (iPhone-style privacy dots) and Caps Lock
//! (PLAN.md stage 6).
//!
//! The registry's `CapabilityAccessManager\ConsentStore` is no longer updated
//! live on current Windows 11 builds (usage moved into a database only SYSTEM can
//! read), so the public APIs are used instead:
//! - microphone: active WASAPI capture sessions on every input device, read once
//!   a second (a handful of COM calls);
//! - camera: Media Foundation's `IMFSensorActivityMonitor`, which reports the
//!   processes streaming from a camera;
//! - Caps Lock: its toggle bit, read every 100 ms (no keyboard hook).

#[cfg(windows)]
mod win32;

use std::sync::{Mutex, PoisonError};
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::error::AppResult;
use crate::thread::spawn_supervised;

const CAPS_POLL: Duration = Duration::from_millis(100);
const SENSORS_POLL: Duration = Duration::from_secs(1);

/// Apps using the microphone and the camera right now. Mirrored in `src/ipc/commands.ts`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Sensors {
    pub microphone: Vec<String>,
    pub camera: Vec<String>,
}

/// Last state seen by the watcher, for `indicators_get`.
static CURRENT: Mutex<Sensors> = Mutex::new(Sensors {
    microphone: Vec::new(),
    camera: Vec::new(),
});

/// `C:\Users\me\AppData\Local\Discord\app-1.0.9\Discord.exe` → `Discord`.
#[cfg_attr(not(windows), allow(dead_code))]
fn exe_name(path: &str) -> String {
    let file = path.rsplit(['\\', '/']).next().unwrap_or(path);
    match file.rsplit_once('.') {
        Some((stem, ext)) if ext.eq_ignore_ascii_case("exe") && !stem.is_empty() => stem.to_owned(),
        _ => file.to_owned(),
    }
}

/// Sorted, without duplicates (apps like Discord or browsers run many processes).
#[cfg_attr(not(windows), allow(dead_code))]
fn tidy(mut names: Vec<String>) -> Vec<String> {
    names.sort_by_key(|n| n.to_lowercase());
    names.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
    names
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
        sensors: CURRENT
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone(),
        caps_lock: caps_lock_on(),
    })
}

fn emit<P: Serialize + Clone>(app: &AppHandle, event: &str, payload: P) {
    if let Err(err) = app.emit(event, payload) {
        log::warn!("failed to emit {event}: {err}");
    }
}

/// Publishes `now` if it differs from what was last seen.
fn publish(app: &AppHandle, now: Sensors) {
    let mut current = CURRENT.lock().unwrap_or_else(PoisonError::into_inner);
    if *current == now {
        return;
    }
    log::info!(
        "sensors in use: microphone {:?}, camera {:?}",
        now.microphone,
        now.camera
    );
    *current = now.clone();
    drop(current);
    emit(app, "indicators://sensors", now);
}

#[cfg(windows)]
fn watch_sensors(app: &AppHandle) {
    use std::collections::BTreeSet;
    use std::sync::Arc;

    win32::init_thread();
    let camera_pids: win32::CameraPids = Arc::new(Mutex::new(BTreeSet::new()));
    // Kept alive for as long as the loop runs.
    let _monitor = match win32::CameraMonitor::start(camera_pids.clone()) {
        Ok(monitor) => Some(monitor),
        Err(err) => {
            log::warn!("camera activity monitor unavailable: {err}");
            None
        }
    };
    let names = |pids: &BTreeSet<u32>| {
        tidy(
            pids.iter()
                .filter_map(|&pid| win32::process_name(pid))
                .collect(),
        )
    };
    loop {
        let microphone = win32::microphone_pids().unwrap_or_else(|err| {
            log::debug!("failed to read capture sessions: {err}");
            BTreeSet::new()
        });
        let camera = camera_pids
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        publish(
            app,
            Sensors {
                microphone: names(&microphone),
                camera: names(&camera),
            },
        );
        std::thread::sleep(SENSORS_POLL);
    }
}

/// Starts the watchers: `indicators://sensors` and `indicators://caps-lock`.
pub fn start(app: &AppHandle) {
    let sensors_app = app.clone();
    spawn_supervised("indicators-sensors", move || {
        #[cfg(windows)]
        watch_sensors(&sensors_app);
        #[cfg(not(windows))]
        publish(&sensors_app, Sensors::default());
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
    fn names_apps_by_exe() {
        assert_eq!(
            exe_name(r"C:\Users\me\AppData\Local\Discord\app-1.0.9\Discord.exe"),
            "Discord"
        );
        assert_eq!(exe_name(r"C:\Tools\obs64.EXE"), "obs64");
        assert_eq!(
            exe_name(r"C:\Program Files\WindowsApps\Microsoft.WindowsCamera\WindowsCamera.exe"),
            "WindowsCamera"
        );
        assert_eq!(exe_name("weird"), "weird");
    }

    #[test]
    fn tidies_duplicates() {
        let names = ["Discord", "browser", "discord"].map(String::from).to_vec();
        assert_eq!(tidy(names), ["browser", "Discord"]);
    }
}
