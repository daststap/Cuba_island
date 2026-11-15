//! Region screenshot with annotation (PLAN.md stage 6).
//!
//! The monitor under the cursor is captured first (so the picture is "frozen"),
//! then a borderless top-most window covering that monitor opens the same page
//! with `?snip`: the user selects a region and draws on it right there. Enter
//! sends the finished RGBA crop back and it goes to the clipboard and history.
//! Pixels travel as raw bytes in both directions — no PNG on the hot path.

#[cfg(windows)]
mod win32;

use std::sync::{Mutex, PoisonError};
use std::time::Duration;

use tauri::ipc::{InvokeBody, Request, Response};
use tauri::{AppHandle, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindow};

use crate::error::{AppError, AppResult};

const LABEL: &str = "snip";
/// Time for the island to fold away when the capture starts from its button.
const COLLAPSE_DELAY: Duration = Duration::from_millis(250);
/// Width and height prefix of raw frames, little-endian u32 each.
const HEADER: usize = 8;

/// The frozen monitor picture, kept until the overlay closes.
#[derive(Default)]
pub struct SnipShared {
    frame: Mutex<Option<Vec<u8>>>,
}

/// Swaps GDI's BGRA into RGBA and makes every pixel opaque.
#[cfg_attr(not(windows), allow(dead_code))]
fn bgra_to_rgba(pixels: &mut [u8]) {
    for px in pixels.chunks_exact_mut(4) {
        px.swap(0, 2);
        px[3] = 255;
    }
}

/// `[width, height]` header + pixels.
fn pack(width: u32, height: u32, rgba: Vec<u8>) -> Vec<u8> {
    let mut out = Vec::with_capacity(HEADER + rgba.len());
    out.extend_from_slice(&width.to_le_bytes());
    out.extend_from_slice(&height.to_le_bytes());
    out.extend(rgba);
    out
}

/// Inverse of `pack`, checking the sizes.
fn unpack(bytes: &[u8]) -> AppResult<(u32, u32, Vec<u8>)> {
    let bad = || AppError::InvalidArgument("повреждённая картинка".into());
    let header = bytes.get(..HEADER).ok_or_else(bad)?;
    let width = u32::from_le_bytes(header[..4].try_into().map_err(|_| bad())?);
    let height = u32::from_le_bytes(header[4..].try_into().map_err(|_| bad())?);
    let pixels = &bytes[HEADER..];
    if width == 0 || height == 0 || pixels.len() != width as usize * height as usize * 4 {
        return Err(bad());
    }
    Ok((width, height, pixels.to_vec()))
}

fn shared(app: &AppHandle) -> tauri::State<'_, SnipShared> {
    app.state::<SnipShared>()
}

pub fn setup(app: &AppHandle) {
    app.manage(SnipShared::default());
}

/// Freezes the monitor under the cursor and opens the overlay on it.
/// Runs off the calling thread: creating a webview from the main thread or a
/// synchronous command can deadlock on Windows.
pub fn start(app: &AppHandle, delay: Duration) {
    let app = app.clone();
    let spawned = std::thread::Builder::new()
        .name("screenshot".into())
        .spawn(move || {
            std::thread::sleep(delay);
            if let Err(err) = open(&app) {
                log::error!("screenshot failed: {err}");
            }
        });
    if let Err(err) = spawned {
        log::error!("failed to start the screenshot: {err}");
    }
}

#[cfg(windows)]
fn open(app: &AppHandle) -> AppResult<()> {
    if let Some(window) = app.get_webview_window(LABEL) {
        // Already open: bring it back instead of stacking a second one.
        return crate::window::bring_to_front(&window);
    }
    let cursor = app.cursor_position()?;
    let monitor = app
        .monitor_from_point(cursor.x, cursor.y)?
        .or(app.primary_monitor()?)
        .ok_or_else(|| AppError::Unavailable("монитор не найден".into()))?;
    let (pos, size) = (*monitor.position(), *monitor.size());
    let rgba = win32::capture(pos.x, pos.y, size.width, size.height)?;
    *shared(app)
        .frame
        .lock()
        .unwrap_or_else(PoisonError::into_inner) = Some(pack(size.width, size.height, rgba));

    let window =
        tauri::WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("index.html?snip".into()))
            .title("Escape")
            .decorations(false)
            .always_on_top(true)
            .skip_taskbar(true)
            .resizable(false)
            .shadow(false)
            .visible(false)
            .build()?;
    window.set_position(PhysicalPosition::new(pos.x, pos.y))?;
    window.set_size(PhysicalSize::new(size.width, size.height))?;
    log::info!(
        "screenshot of {}×{} at ({}, {})",
        size.width,
        size.height,
        pos.x,
        pos.y
    );
    Ok(())
}

#[cfg(not(windows))]
fn open(_app: &AppHandle) -> AppResult<()> {
    Err(AppError::Unavailable(
        "скриншоты есть только в Windows".into(),
    ))
}

fn close(app: &AppHandle) -> AppResult<()> {
    *shared(app)
        .frame
        .lock()
        .unwrap_or_else(PoisonError::into_inner) = None;
    if let Some(window) = app.get_webview_window(LABEL) {
        window.destroy()?;
    }
    Ok(())
}

/// The island's button: lets the panel fold away, then captures.
#[tauri::command]
pub fn screenshot_start(app: AppHandle) -> AppResult<()> {
    start(&app, COLLAPSE_DELAY);
    Ok(())
}

/// The frozen picture: 8-byte `[width, height]` header, then RGBA pixels.
#[tauri::command]
pub fn screenshot_frame(app: AppHandle) -> AppResult<Response> {
    let frame = shared(&app)
        .frame
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clone()
        .ok_or_else(|| AppError::Unavailable("снимок уже закрыт".into()))?;
    Ok(Response::new(frame))
}

/// The page has drawn the picture: show the overlay and take keyboard focus.
#[tauri::command]
pub fn screenshot_ready(window: WebviewWindow) -> AppResult<()> {
    window.show()?;
    crate::window::bring_to_front(&window)
}

/// Copies the finished image (raw body in the `pack` format) and closes.
#[tauri::command]
pub fn screenshot_copy(request: Request<'_>, app: AppHandle) -> AppResult<()> {
    let InvokeBody::Raw(bytes) = request.body() else {
        return Err(AppError::InvalidArgument("ожидалась картинка".into()));
    };
    let (width, height, rgba) = unpack(bytes)?;
    // Close first: the overlay must not linger if the clipboard is busy.
    close(&app)?;
    crate::clipboard::copy_image(&app, width, height, rgba)
}

/// Esc: closes without copying.
#[tauri::command]
pub fn screenshot_close(app: AppHandle) -> AppResult<()> {
    close(&app)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn swaps_channels_and_makes_opaque() {
        let mut px = vec![1, 2, 3, 0, 10, 20, 30, 7];
        bgra_to_rgba(&mut px);
        assert_eq!(px, [3, 2, 1, 255, 30, 20, 10, 255]);
    }

    #[test]
    fn packs_and_unpacks_frames() {
        let rgba = vec![9; 2 * 3 * 4];
        let packed = pack(2, 3, rgba.clone());
        assert_eq!(packed.len(), HEADER + rgba.len());
        assert_eq!(unpack(&packed).unwrap(), (2, 3, rgba));
    }

    #[test]
    fn rejects_broken_frames() {
        assert!(unpack(&[1, 0, 0]).is_err());
        assert!(unpack(&pack(2, 2, vec![0; 3])).is_err());
        assert!(unpack(&pack(0, 0, Vec::new())).is_err());
    }
}
