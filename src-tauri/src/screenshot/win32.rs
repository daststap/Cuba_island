//! Screen capture with GDI.

use std::mem::size_of;

use windows::Win32::Graphics::Gdi::{
    BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetDC, GetDIBits,
    ReleaseDC, SelectObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, SRCCOPY,
};

use super::bgra_to_rgba;
use crate::error::{AppError, AppResult};

/// Copies the screen rectangle (physical pixels, virtual-desktop coordinates)
/// as top-down RGBA.
pub fn capture(x: i32, y: i32, width: u32, height: u32) -> AppResult<Vec<u8>> {
    let (w, h) = (
        i32::try_from(width).map_err(|_| AppError::Internal("слишком широкий экран".into()))?,
        i32::try_from(height).map_err(|_| AppError::Internal("слишком высокий экран".into()))?,
    );
    let mut pixels = vec![0u8; width as usize * height as usize * 4];
    // SAFETY: every GDI object created here is released before returning; the
    // bitmap is deselected before it is deleted; `pixels` holds exactly
    // width × height 32-bit pixels, matching the BITMAPINFO passed to GetDIBits.
    let lines = unsafe {
        let screen = GetDC(None);
        if screen.is_invalid() {
            return Err(AppError::Internal("нет доступа к экрану".into()));
        }
        let memory = CreateCompatibleDC(Some(screen));
        let bitmap = CreateCompatibleBitmap(screen, w, h);
        let previous = SelectObject(memory, bitmap.into());
        let copied = BitBlt(memory, 0, 0, w, h, Some(screen), x, y, SRCCOPY);
        SelectObject(memory, previous);
        let mut info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: w,
                // Negative height: rows top to bottom.
                biHeight: -h,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let lines = if copied.is_ok() {
            GetDIBits(
                memory,
                bitmap,
                0,
                height,
                Some(pixels.as_mut_ptr().cast()),
                &mut info,
                DIB_RGB_COLORS,
            )
        } else {
            0
        };
        let _ = DeleteObject(bitmap.into());
        let _ = DeleteDC(memory);
        ReleaseDC(None, screen);
        lines
    };
    if lines != h {
        return Err(AppError::Internal("не удалось снять экран".into()));
    }
    bgra_to_rgba(&mut pixels);
    Ok(pixels)
}
