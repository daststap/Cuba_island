//! Chat avatars for notifications. The official listener API gives no images,
//! so they are read (read-only) from Windows' own notification database, where
//! each toast's XML may carry the sender's picture as `appLogoOverride`.
//! Undocumented: anything unexpected just means "no avatar" (the app icon is used).
//! Only local files are used — never a web link.

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

/// Avatars not seen for this long are deleted from the cache.
const KEEP: Duration = Duration::from_secs(7 * 24 * 60 * 60);
/// Pictures bigger than this are not avatars.
const MAX_BYTES: u64 = 2 * 1024 * 1024;

/// The `src` of the `<image placement="appLogoOverride">` in a toast's XML.
pub fn logo_src(xml: &str) -> Option<String> {
    let mut rest = xml;
    while let Some(start) = rest.find("<image") {
        let tag_end = rest[start..].find('>').map_or(rest.len(), |e| start + e);
        let tag = &rest[start..tag_end];
        if tag.contains("appLogoOverride") {
            return attribute(tag, "src");
        }
        rest = &rest[tag_end..];
    }
    None
}

fn attribute(tag: &str, name: &str) -> Option<String> {
    for quote in ['"', '\''] {
        let key = format!("{name}={quote}");
        if let Some(at) = tag.find(&key) {
            let value = &tag[at + key.len()..];
            return value.find(quote).map(|end| unescape(&value[..end]));
        }
    }
    None
}

fn unescape(text: &str) -> String {
    text.replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
}

/// A local file behind `src` (`file:///C:/…` or a plain path); `None` for web
/// links and app-relative URIs.
pub fn local_path(src: &str) -> Option<PathBuf> {
    let path = if let Some(rest) = src.strip_prefix("file:///") {
        percent_decode(rest).replace('/', "\\")
    } else if src.len() > 2 && src.as_bytes()[1] == b':' {
        src.to_owned()
    } else {
        return None;
    };
    Some(PathBuf::from(path))
}

fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(byte) = u8::from_str_radix(&text[i + 1..i + 3], 16) {
                out.push(byte);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Reads the avatars of `ids` and keeps copies in `cache_dir` (apps delete
/// their temporary pictures). Returns id → the picture as a data URL.
pub fn load(ids: &[u32], cache_dir: &Path) -> HashMap<u32, String> {
    let mut found = HashMap::new();
    if ids.is_empty() {
        return found;
    }
    let Some(local) = std::env::var_os("LOCALAPPDATA") else {
        return found;
    };
    let db = PathBuf::from(local).join(r"Microsoft\Windows\Notifications\wpndatabase.db");
    let conn = match rusqlite::Connection::open_with_flags(
        &db,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    ) {
        Ok(conn) => conn,
        Err(err) => {
            log::debug!("notification database unavailable: {err}");
            return found;
        }
    };
    let Ok(mut query) = conn.prepare("SELECT Payload FROM Notification WHERE Id = ?1") else {
        return found;
    };
    for &id in ids {
        let payload = query
            .query_row([id], |row| {
                row.get::<_, Vec<u8>>(0)
                    .or_else(|_| row.get::<_, String>(0).map(String::into_bytes))
            })
            .ok();
        let src = payload.and_then(|bytes| logo_src(&String::from_utf8_lossy(&bytes)));
        if let Some(cached) = src
            .and_then(|s| local_path(&s))
            .and_then(|p| keep(&p, cache_dir))
        {
            found.insert(id, cached);
        }
    }
    log::info!(
        "avatars: {} of {} new notifications",
        found.len(),
        ids.len()
    );
    found
}

/// Copies a picture into the cache under a content-based name; returns it as a
/// data URL (the page shows it without the asset protocol).
fn keep(source: &Path, cache_dir: &Path) -> Option<String> {
    let size = std::fs::metadata(source).ok()?.len();
    if size == 0 || size > MAX_BYTES {
        return None;
    }
    let bytes = std::fs::read(source).ok()?;
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    bytes.hash(&mut hasher);
    let ext = source
        .extension()
        .and_then(|e| e.to_str())
        .filter(|e| e.len() <= 4)
        .unwrap_or("png");
    let target = cache_dir.join(format!("avatar-{:016x}.{ext}", hasher.finish()));
    if !target.exists() {
        std::fs::create_dir_all(cache_dir).ok()?;
        std::fs::write(&target, &bytes).ok()?;
    }
    crate::util::data_url(&target)
}

/// Deletes cached avatars older than a week.
pub fn prune(cache_dir: &Path) {
    let Ok(entries) = std::fs::read_dir(cache_dir) else {
        return;
    };
    let now = SystemTime::now();
    for entry in entries.flatten() {
        let name = entry.file_name();
        if !name.to_string_lossy().starts_with("avatar-") {
            continue;
        }
        let old = entry
            .metadata()
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| now.duration_since(t).ok())
            .is_some_and(|age| age > KEEP);
        if old {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_app_logo_override_image() {
        let xml = r#"<toast><visual><binding template="ToastGeneric">
            <image src="file:///C:/other.png" placement="hero"/>
            <image hint-crop='circle' placement="appLogoOverride" src="file:///C:/Users/me/AppData/Roaming/Telegram%20Desktop/tdata/temp/pic&amp;1.png"/>
            <text>Маша</text></binding></visual></toast>"#;
        assert_eq!(
            logo_src(xml).as_deref(),
            Some("file:///C:/Users/me/AppData/Roaming/Telegram%20Desktop/tdata/temp/pic&1.png")
        );
        assert_eq!(logo_src("<toast><visual/></toast>"), None);
    }

    #[test]
    fn uses_only_local_files() {
        assert_eq!(
            local_path("file:///C:/Users/me/Telegram%20Desktop/pic.png"),
            Some(PathBuf::from(r"C:\Users\me\Telegram Desktop\pic.png"))
        );
        assert_eq!(
            local_path(r"C:\temp\a.png"),
            Some(PathBuf::from(r"C:\temp\a.png"))
        );
        assert_eq!(local_path("https://example.com/a.png"), None);
        assert_eq!(local_path("ms-appdata:///local/a.png"), None);
    }
}
