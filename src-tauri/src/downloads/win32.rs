//! Where downloads go: the user's Downloads folder and Steam's install folder.

use std::path::PathBuf;

use windows::core::w;
use windows::Win32::Foundation::ERROR_SUCCESS;
use windows::Win32::System::Com::CoTaskMemFree;
use windows::Win32::System::Registry::{RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_SZ};
use windows::Win32::UI::Shell::{FOLDERID_Downloads, SHGetKnownFolderPath, KF_FLAG_DEFAULT};

/// Longest registry string we read, in UTF-16 units.
const MAX_VALUE: usize = 1024;

/// The Downloads known folder (follows the user if they moved it).
pub fn downloads_dir() -> Option<PathBuf> {
    // SAFETY: on success the returned string is owned by us and freed below.
    let path = unsafe { SHGetKnownFolderPath(&FOLDERID_Downloads, KF_FLAG_DEFAULT, None) }.ok()?;
    // SAFETY: SHGetKnownFolderPath returns a valid NUL-terminated string.
    let text = unsafe { path.to_string() }.ok();
    // SAFETY: the buffer was allocated by the shell with CoTaskMemAlloc.
    unsafe { CoTaskMemFree(Some(path.0.cast())) };
    text.map(PathBuf::from)
}

/// Steam's folder from `HKCU\Software\Valve\Steam\SteamPath`, if Steam is installed.
pub fn steam_dir() -> Option<PathBuf> {
    let mut buf = [0u16; MAX_VALUE];
    let mut size = (buf.len() * 2) as u32;
    // SAFETY: `buf` is writable for `size` bytes; the strings are static.
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            w!(r"Software\Valve\Steam"),
            w!("SteamPath"),
            RRF_RT_REG_SZ,
            None,
            Some(buf.as_mut_ptr().cast()),
            Some(&mut size),
        )
    };
    if status != ERROR_SUCCESS {
        return None;
    }
    // `size` counts bytes including the terminating NUL.
    let len = (size as usize / 2).saturating_sub(1);
    let text = String::from_utf16_lossy(&buf[..len.min(buf.len())]);
    (!text.is_empty()).then(|| PathBuf::from(text))
}
