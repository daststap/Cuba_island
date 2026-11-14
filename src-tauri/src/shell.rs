//! Windows shell helpers shared by the launcher and the file shelf: item images
//! (icons or thumbnails) and dragging files out to other apps.

use std::path::{Path, PathBuf};

use windows::core::{Interface, HSTRING};
use windows::Win32::Foundation::{HWND, SIZE};
use windows::Win32::Graphics::Gdi::{
    DeleteObject, GetDC, GetDIBits, GetObjectW, ReleaseDC, BITMAP, BITMAPINFO, BITMAPINFOHEADER,
    BI_RGB, DIB_RGB_COLORS, HBITMAP,
};
use windows::Win32::System::Com::{CoTaskMemFree, IDataObject};
use windows::Win32::System::Ole::{IDropSource, DROPEFFECT_COPY, DROPEFFECT_LINK, DROPEFFECT_NONE};
use windows::Win32::UI::Shell::Common::ITEMIDLIST;
use windows::Win32::UI::Shell::{
    BHID_DataObject, IShellItem, IShellItemArray, IShellItemImageFactory,
    SHCreateItemFromParsingName, SHCreateShellItemArrayFromIDLists, SHDoDragDrop,
    SHParseDisplayName, SIIGBF_BIGGERSIZEOK, SIIGBF_ICONONLY, SIIGBF_RESIZETOFIT,
};

pub type ShellResult<T> = Result<T, Box<dyn std::error::Error>>;

/// The shell item for a file or folder path. COM must be initialised.
pub fn item_from_path(path: &Path) -> windows::core::Result<IShellItem> {
    let path = HSTRING::from(path.as_os_str());
    // SAFETY: `path` is a valid wide string for the duration of the call.
    unsafe { SHCreateItemFromParsingName(&path, None) }
}

/// Saves the item's image as a PNG: its icon, or (`icon_only = false`) a thumbnail
/// for pictures and videos where Windows has one. COM must be initialised.
pub fn save_image(item: &IShellItem, path: &Path, size: i32, icon_only: bool) -> ShellResult<()> {
    let factory: IShellItemImageFactory = item.cast()?;
    let flags = if icon_only {
        SIIGBF_ICONONLY | SIIGBF_BIGGERSIZEOK
    } else {
        SIIGBF_RESIZETOFIT | SIIGBF_BIGGERSIZEOK
    };
    // SAFETY: the factory returns an HBITMAP we own and delete below.
    let bitmap = unsafe { factory.GetImage(SIZE { cx: size, cy: size }, flags)? };
    let pixels = read_bitmap(bitmap);
    // SAFETY: deleting the bitmap we own.
    let _ = unsafe { DeleteObject(bitmap.into()) };
    let (width, height, rgba) = pixels.ok_or("unreadable shell bitmap")?;
    image::save_buffer(path, &rgba, width, height, image::ExtendedColorType::Rgba8)?;
    Ok(())
}

/// Copies a 32-bit premultiplied BGRA bitmap into straight RGBA.
fn read_bitmap(bitmap: HBITMAP) -> Option<(u32, u32, Vec<u8>)> {
    let mut info = BITMAP::default();
    // SAFETY: `info` is a writable BITMAP and the size matches.
    let read = unsafe {
        GetObjectW(
            bitmap.into(),
            std::mem::size_of::<BITMAP>() as i32,
            Some((&mut info as *mut BITMAP).cast()),
        )
    };
    if read == 0 || info.bmWidth <= 0 || info.bmHeight <= 0 {
        return None;
    }
    let (width, height) = (info.bmWidth, info.bmHeight);
    let mut header = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: width,
            biHeight: -height, // top-down rows
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut buffer = vec![0_u8; (width * height * 4) as usize];
    // SAFETY: the screen DC is released below; `buffer` holds width*height 32-bit pixels,
    // exactly what the header asks GetDIBits to write.
    let copied = unsafe {
        let dc = GetDC(None);
        let copied = GetDIBits(
            dc,
            bitmap,
            0,
            height as u32,
            Some(buffer.as_mut_ptr().cast()),
            &mut header,
            DIB_RGB_COLORS,
        );
        ReleaseDC(None, dc);
        copied
    };
    if copied == 0 {
        return None;
    }
    to_straight_rgba(&mut buffer);
    Some((width as u32, height as u32, buffer))
}

/// BGRA (premultiplied) → RGBA (straight). Thumbnails of photos often come with
/// alpha 0 everywhere, meaning "opaque", not "invisible".
fn to_straight_rgba(buffer: &mut [u8]) {
    let no_alpha = buffer.chunks_exact(4).all(|px| px[3] == 0);
    for px in buffer.chunks_exact_mut(4) {
        px.swap(0, 2);
        if no_alpha {
            px[3] = 255;
            continue;
        }
        let alpha = u16::from(px[3]);
        if alpha > 0 && alpha < 255 {
            for channel in &mut px[..3] {
                *channel = ((u16::from(*channel) * 255 + alpha / 2) / alpha).min(255) as u8;
            }
        }
    }
}

/// Starts a shell drag of `paths` (as Explorer does) and blocks until it ends.
/// Must run on the window's (main) thread while the mouse button is down.
/// Returns whether the files were dropped somewhere.
pub fn drag_files(hwnd: HWND, paths: &[PathBuf]) -> ShellResult<bool> {
    let mut pidls: Vec<*mut ITEMIDLIST> = Vec::with_capacity(paths.len());
    let result = (|| -> ShellResult<bool> {
        for path in paths {
            let mut pidl = std::ptr::null_mut();
            let wide = HSTRING::from(path.as_os_str());
            // SAFETY: `pidl` receives a shell-allocated ID list, freed below.
            unsafe { SHParseDisplayName(&wide, None, &mut pidl, 0, None)? };
            pidls.push(pidl);
        }
        let list: Vec<*const ITEMIDLIST> = pidls.iter().map(|p| p.cast_const()).collect();
        // SAFETY: every pointer in `list` is a valid ID list owned by `pidls`.
        let items: IShellItemArray = unsafe { SHCreateShellItemArrayFromIDLists(&list)? };
        // SAFETY: binding the item array to the shell's own data object for these files.
        let data: IDataObject = unsafe { items.BindToHandler(None, &BHID_DataObject)? };
        // SAFETY: modal OLE drag loop on the window's thread; `None` = the shell's
        // default drop source (cursor feedback and drag image like Explorer).
        let effect = unsafe {
            SHDoDragDrop(
                Some(hwnd),
                &data,
                None::<&IDropSource>,
                DROPEFFECT_COPY | DROPEFFECT_LINK,
            )?
        };
        Ok(effect != DROPEFFECT_NONE)
    })();
    for pidl in pidls {
        // SAFETY: freeing ID lists allocated by SHParseDisplayName.
        unsafe { CoTaskMemFree(Some(pidl as *const _)) };
    }
    result
}

#[cfg(test)]
mod tests {
    use super::to_straight_rgba;

    #[test]
    fn converts_bgra_and_unpremultiplies() {
        let mut px = vec![10, 20, 30, 255, 0, 0, 64, 128];
        to_straight_rgba(&mut px);
        assert_eq!(&px[..4], &[30, 20, 10, 255]);
        assert_eq!(&px[4..], &[128, 0, 0, 128]);
    }

    #[test]
    fn treats_all_zero_alpha_as_opaque() {
        let mut px = vec![1, 2, 3, 0, 4, 5, 6, 0];
        to_straight_rgba(&mut px);
        assert_eq!(px, vec![3, 2, 1, 255, 6, 5, 4, 255]);
    }
}
