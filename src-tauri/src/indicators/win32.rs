//! Who is using the microphone (WASAPI capture sessions) and the camera
//! (Media Foundation sensor activity monitor), and the Caps Lock state.

use std::collections::BTreeSet;
use std::sync::{Arc, Mutex, PoisonError};

use windows::core::{implement, Ref, PWSTR};
use windows::Win32::Foundation::{CloseHandle, S_OK};
use windows::Win32::Media::Audio::{
    eCapture, AudioSessionStateActive, IAudioSessionControl2, IAudioSessionManager2,
    IMMDeviceEnumerator, MMDeviceEnumerator, DEVICE_STATE_ACTIVE,
};
use windows::Win32::Media::MediaFoundation::{
    IMFSensorActivitiesReport, IMFSensorActivitiesReportCallback,
    IMFSensorActivitiesReportCallback_Impl, IMFSensorActivityMonitor,
    MFCreateSensorActivityMonitor, MFStartup, MFSTARTUP_LITE, MF_VERSION,
};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CLSCTX_ALL, COINIT_MULTITHREADED,
};
use windows::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{GetKeyState, VK_CAPITAL};

use super::exe_name;

/// Longest process image path we read.
const MAX_PATH_LEN: usize = 1024;

/// Name of the program running as `pid` (`Discord`), if it can be read.
pub fn process_name(pid: u32) -> Option<String> {
    // SAFETY: plain handle query; the handle is closed below.
    let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }.ok()?;
    let mut buf = [0u16; MAX_PATH_LEN];
    let mut len = buf.len() as u32;
    // SAFETY: `buf` is writable for `len` UTF-16 units; the handle is valid.
    let read = unsafe {
        QueryFullProcessImageNameW(
            process,
            PROCESS_NAME_WIN32,
            PWSTR(buf.as_mut_ptr()),
            &mut len,
        )
    };
    // SAFETY: the handle came from OpenProcess and is closed exactly once.
    let _ = unsafe { CloseHandle(process) };
    read.ok()?;
    Some(exe_name(&String::from_utf16_lossy(&buf[..len as usize])))
}

/// Prepares COM (multithreaded) and Media Foundation on the calling thread.
pub fn init_thread() {
    // SAFETY: initialises COM for this thread; repeated calls are harmless.
    let _ = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
    // SAFETY: plain library start-up; balanced by process exit.
    if let Err(err) = unsafe { MFStartup(MF_VERSION, MFSTARTUP_LITE) } {
        log::warn!("Media Foundation did not start: {err}");
    }
}

/// Processes with an active capture (recording) session on any input device.
pub fn microphone_pids() -> windows::core::Result<BTreeSet<u32>> {
    let own = std::process::id();
    let mut pids = BTreeSet::new();
    // SAFETY: COM is initialised on this thread by `init_thread`; all interfaces
    // below are reference-counted and released on drop.
    unsafe {
        let devices: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
        let inputs = devices.EnumAudioEndpoints(eCapture, DEVICE_STATE_ACTIVE)?;
        for d in 0..inputs.GetCount()? {
            let Ok(device) = inputs.Item(d) else { continue };
            let Ok(manager) = device.Activate::<IAudioSessionManager2>(CLSCTX_ALL, None) else {
                continue;
            };
            let Ok(sessions) = manager.GetSessionEnumerator() else {
                continue;
            };
            for s in 0..sessions.GetCount()? {
                let Ok(session) = sessions.GetSession(s) else {
                    continue;
                };
                if session.GetState() != Ok(AudioSessionStateActive) {
                    continue;
                }
                let Ok(session) = windows::core::Interface::cast::<IAudioSessionControl2>(&session)
                else {
                    continue;
                };
                if session.IsSystemSoundsSession() == S_OK {
                    continue;
                }
                if let Ok(pid) = session.GetProcessId() {
                    if pid != 0 && pid != own {
                        pids.insert(pid);
                    }
                }
            }
        }
    }
    Ok(pids)
}

/// Processes streaming from a camera, kept up to date by the sensor monitor.
pub type CameraPids = Arc<Mutex<BTreeSet<u32>>>;

#[implement(IMFSensorActivitiesReportCallback)]
struct CameraReports {
    pids: CameraPids,
}

impl IMFSensorActivitiesReportCallback_Impl for CameraReports_Impl {
    fn OnActivitiesReport(
        &self,
        report: Ref<IMFSensorActivitiesReport>,
    ) -> windows::core::Result<()> {
        let report = report.ok()?;
        let mut streaming = BTreeSet::new();
        // SAFETY: the report is valid for the duration of the callback.
        unsafe {
            for d in 0..report.GetCount()? {
                let Ok(device) = report.GetActivityReport(d) else {
                    continue;
                };
                for p in 0..device.GetProcessCount()? {
                    let Ok(process) = device.GetProcessActivity(p) else {
                        continue;
                    };
                    if process.GetStreamingState().is_ok_and(|s| s.as_bool()) {
                        if let Ok(pid) = process.GetProcessId() {
                            streaming.insert(pid);
                        }
                    }
                }
            }
        }
        *self.pids.lock().unwrap_or_else(PoisonError::into_inner) = streaming;
        Ok(())
    }
}

/// Starts watching the cameras; keep the monitor alive (it stops on drop).
pub struct CameraMonitor(IMFSensorActivityMonitor);

impl CameraMonitor {
    pub fn start(pids: CameraPids) -> windows::core::Result<Self> {
        let callback: IMFSensorActivitiesReportCallback = CameraReports { pids }.into();
        // SAFETY: Media Foundation is started by `init_thread`; the callback object
        // is reference-counted and kept by the monitor.
        let monitor = unsafe { MFCreateSensorActivityMonitor(&callback)? };
        // SAFETY: starts the monitor's own worker; stopped in `Drop`.
        unsafe { monitor.Start()? };
        Ok(Self(monitor))
    }
}

impl Drop for CameraMonitor {
    fn drop(&mut self) {
        // SAFETY: the monitor was started in `start`.
        let _ = unsafe { self.0.Stop() };
    }
}

/// Whether Caps Lock is on (the toggle bit of its key state).
pub fn caps_lock_on() -> bool {
    // SAFETY: plain query of the keyboard state; no pointers involved.
    let state = unsafe { GetKeyState(i32::from(VK_CAPITAL.0)) };
    state & 1 != 0
}
