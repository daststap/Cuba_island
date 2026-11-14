//! System information: battery.

use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::error::AppResult;
use crate::thread::spawn_supervised;

const BATTERY_POLL: Duration = Duration::from_secs(30);

/// `BatteryFlag` bit meaning the machine has no system battery.
const FLAG_NO_BATTERY: u8 = 128;
/// `BatteryFlag` bit meaning the battery is charging.
const FLAG_CHARGING: u8 = 8;
/// `BatteryFlag` / `BatteryLifePercent` value meaning "unknown".
const UNKNOWN: u8 = 255;
/// `ACLineStatus` value meaning the charger is connected.
const AC_ONLINE: u8 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Battery {
    pub percent: u8,
    pub charging: bool,
    pub plugged_in: bool,
}

/// Interprets `SYSTEM_POWER_STATUS` fields; `None` when there is no usable battery.
#[cfg_attr(not(windows), allow(dead_code))]
fn battery_from_status(ac_line: u8, flag: u8, percent: u8) -> Option<Battery> {
    if flag == UNKNOWN || flag & FLAG_NO_BATTERY != 0 || percent > 100 {
        return None;
    }
    Some(Battery {
        percent,
        charging: flag & FLAG_CHARGING != 0,
        plugged_in: ac_line == AC_ONLINE,
    })
}

#[cfg(windows)]
fn read_battery() -> Option<Battery> {
    use windows::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};

    let mut status = SYSTEM_POWER_STATUS::default();
    // SAFETY: `status` is a valid, writable SYSTEM_POWER_STATUS for the duration of the call.
    unsafe { GetSystemPowerStatus(&mut status) }.ok()?;
    battery_from_status(
        status.ACLineStatus,
        status.BatteryFlag,
        status.BatteryLifePercent,
    )
}

#[cfg(not(windows))]
fn read_battery() -> Option<Battery> {
    None
}

#[tauri::command]
pub fn get_battery() -> AppResult<Option<Battery>> {
    Ok(read_battery())
}

/// Checks the battery every 30 s and emits `system://battery` when it changed.
pub fn spawn_battery_watcher(app: AppHandle) {
    spawn_supervised("system-battery", move || {
        let mut last = read_battery();
        loop {
            std::thread::sleep(BATTERY_POLL);
            let now = read_battery();
            if now != last {
                last = now;
                if let Err(err) = app.emit("system://battery", now) {
                    log::warn!("failed to emit system://battery: {err}");
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_battery_on_desktops() {
        assert_eq!(battery_from_status(1, FLAG_NO_BATTERY, UNKNOWN), None);
        assert_eq!(battery_from_status(1, UNKNOWN, 50), None);
    }

    #[test]
    fn reads_charging_laptop() {
        assert_eq!(
            battery_from_status(AC_ONLINE, FLAG_CHARGING, 42),
            Some(Battery {
                percent: 42,
                charging: true,
                plugged_in: true
            })
        );
    }

    #[test]
    fn reads_discharging_laptop() {
        assert_eq!(
            battery_from_status(0, 0, 87),
            Some(Battery {
                percent: 87,
                charging: false,
                plugged_in: false
            })
        );
    }
}
