//! Index of launchable apps from `shell:AppsFolder` (PLAN.md stage 4).
//!
//! AppsFolder lists everything the Start menu shows: `.lnk` shortcuts from both
//! Start Menu folders *and* Store apps, each with a parsing name (an AUMID or a
//! known-folder path) that `explorer.exe shell:AppsFolder\<name>` can launch.

use std::path::{Path, PathBuf};

use windows::core::{Interface, HSTRING, PWSTR};

use super::SETTINGS_AUMID;
use windows::Win32::Foundation::SIZE;
use windows::Win32::Graphics::Gdi::{
    DeleteObject, GetDC, GetDIBits, GetObjectW, ReleaseDC, BITMAP, BITMAPINFO, BITMAPINFOHEADER,
    BI_RGB, DIB_RGB_COLORS, HBITMAP,
};
use windows::Win32::System::Com::CoTaskMemFree;
use windows::Win32::UI::Shell::{
    BHID_EnumItems, FOLDERID_AppsFolder, IEnumShellItems, IShellItem, IShellItemImageFactory,
    SHCreateItemFromParsingName, SHGetKnownFolderItem, KF_FLAG_DEFAULT, SIGDN, SIGDN_NORMALDISPLAY,
    SIGDN_PARENTRELATIVEPARSING, SIIGBF_BIGGERSIZEOK, SIIGBF_ICONONLY,
};

/// Icons are shown at 32 logical px; 48 keeps them sharp at 150%.
const ICON_SIZE: i32 = 48;

#[derive(Clone, Debug)]
pub struct App {
    pub name: String,
    /// What follows `shell:AppsFolder\` when launching.
    pub parsing_name: String,
    pub icon: Option<PathBuf>,
}

/// Lists all apps. COM must be initialised (STA) on the calling thread.
pub fn scan(icon_dir: &Path) -> windows::core::Result<Vec<App>> {
    // SAFETY: COM is initialised by the caller; arguments are valid constants.
    let folder: IShellItem =
        unsafe { SHGetKnownFolderItem(&FOLDERID_AppsFolder, KF_FLAG_DEFAULT, None)? };
    // SAFETY: binding a shell folder to its item enumerator.
    let items: IEnumShellItems = unsafe { folder.BindToHandler(None, &BHID_EnumItems)? };

    let _ = std::fs::create_dir_all(icon_dir);
    let mut apps = Vec::new();
    loop {
        let mut batch = [None];
        let mut fetched = 0;
        // SAFETY: `batch` has room for one item and `fetched` is a valid out pointer.
        let _ = unsafe { items.Next(&mut batch, Some(&mut fetched)) };
        let Some(item) = batch[0].take().filter(|_| fetched == 1) else {
            break;
        };
        apps.extend(to_app(&item, icon_dir));
    }

    // Windows Settings is not always listed in AppsFolder, but people expect to find it.
    if !apps.iter().any(|app| app.parsing_name == SETTINGS_AUMID) {
        let path = HSTRING::from(format!("shell:AppsFolder\\{SETTINGS_AUMID}"));
        // SAFETY: COM is initialised; the path is a valid wide string for the call.
        let item: windows::core::Result<IShellItem> =
            unsafe { SHCreateItemFromParsingName(&path, None) };
        match item {
            Ok(item) => apps.extend(to_app(&item, icon_dir).map(|app| App {
                parsing_name: SETTINGS_AUMID.to_owned(),
                ..app
            })),
            Err(err) => log::debug!("Windows Settings not found: {err}"),
        }
    }

    apps.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(apps)
}

fn to_app(item: &IShellItem, icon_dir: &Path) -> Option<App> {
    let name = display_name(item, SIGDN_NORMALDISPLAY)?;
    let parsing_name = display_name(item, SIGDN_PARENTRELATIVEPARSING)?;
    // Web links pinned to Start are not apps; uninstallers only clutter the results.
    if name.trim().is_empty() || parsing_name.starts_with("http") || is_uninstaller(&name) {
        return None;
    }
    let icon = icon_path(icon_dir, &parsing_name);
    let icon = (icon.exists() || save_icon(item, &icon).is_ok()).then_some(icon);
    Some(App {
        name,
        parsing_name,
        icon,
    })
}

fn is_uninstaller(name: &str) -> bool {
    let lower = name.to_lowercase();
    ["uninstall", "деинсталл", "удалить ", "удаление "]
        .iter()
        .any(|word| lower.starts_with(word))
}

fn display_name(item: &IShellItem, kind: SIGDN) -> Option<String> {
    // SAFETY: on success the shell allocates a string that we free with CoTaskMemFree.
    unsafe {
        let raw: PWSTR = item.GetDisplayName(kind).ok()?;
        let text = raw.to_string().ok();
        CoTaskMemFree(Some(raw.0 as *const _));
        text
    }
}

fn icon_path(dir: &Path, parsing_name: &str) -> PathBuf {
    dir.join(format!(
        "{:016x}.png",
        crate::util::fnv1a(parsing_name.as_bytes())
    ))
}

fn save_icon(item: &IShellItem, path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let factory: IShellItemImageFactory = item.cast()?;
    let size = SIZE {
        cx: ICON_SIZE,
        cy: ICON_SIZE,
    };
    // SAFETY: the factory returns an HBITMAP we own and delete below.
    let bitmap = unsafe { factory.GetImage(size, SIIGBF_ICONONLY | SIIGBF_BIGGERSIZEOK)? };
    let pixels = read_bitmap(bitmap);
    // SAFETY: deleting the bitmap we own.
    let _ = unsafe { DeleteObject(bitmap.into()) };
    let (width, height, rgba) = pixels.ok_or("unreadable icon bitmap")?;
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
    for px in buffer.chunks_exact_mut(4) {
        px.swap(0, 2); // BGRA -> RGBA
        let alpha = u16::from(px[3]);
        if alpha > 0 && alpha < 255 {
            for channel in &mut px[..3] {
                *channel = ((u16::from(*channel) * 255 + alpha / 2) / alpha).min(255) as u8;
            }
        }
    }
    Some((width as u32, height as u32, buffer))
}

#[cfg(test)]
mod tests {
    use super::is_uninstaller;

    #[test]
    fn recognises_uninstallers() {
        assert!(is_uninstaller("Деинсталлировать Telegram"));
        assert!(is_uninstaller("Uninstall Foo"));
        assert!(is_uninstaller("Удалить Bar"));
        assert!(!is_uninstaller("Telegram"));
        assert!(!is_uninstaller("Удалёнка"));
    }
}
