//! Platform-independent media types and helpers.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use serde::{Deserialize, Serialize};

/// What is playing right now (PLAN.md §5, stage 2). Mirrored in `src/ipc/commands.ts`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaState {
    /// Stable id of the track (hash of app + metadata); changes when the track changes.
    pub track_id: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    /// AppUserModelID of the player, e.g. `Spotify.exe` or `MSEdge`.
    pub app_id: String,
    pub is_playing: bool,
    pub position_ms: u64,
    pub duration_ms: u64,
    /// Unix ms at which `position_ms` was measured; the frontend interpolates from it.
    pub position_updated_at: i64,
    /// `data:image/jpeg;base64,…`, at most ~300 KB.
    pub cover: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MediaCommand {
    Toggle,
    Next,
    Prev,
}

#[cfg_attr(not(windows), allow(dead_code))]
pub fn track_id(app_id: &str, title: &str, artist: &str, album: &str) -> String {
    let mut hasher = DefaultHasher::new();
    (app_id, title, artist, album).hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

/// WinRT `TimeSpan` ticks (100 ns) to milliseconds, clamped at zero.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn ticks_to_ms(ticks: i64) -> u64 {
    u64::try_from(ticks / 10_000).unwrap_or(0)
}

/// Ticks between 1601-01-01 (FILETIME epoch) and 1970-01-01.
const FILETIME_UNIX_EPOCH_TICKS: i64 = 116_444_736_000_000_000;

/// WinRT `DateTime` (FILETIME ticks) to Unix ms; `None` for unset/invalid values.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn filetime_to_unix_ms(ticks: i64) -> Option<i64> {
    (ticks > FILETIME_UNIX_EPOCH_TICKS).then(|| (ticks - FILETIME_UNIX_EPOCH_TICKS) / 10_000)
}

/// Scales `(w, h)` to fit in a `max`×`max` square, keeping the aspect ratio, never upscaling.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn fit_within(w: u32, h: u32, max: u32) -> (u32, u32) {
    if w <= max && h <= max || w == 0 || h == 0 {
        return (w, h);
    }
    let (w64, h64, max64) = (u64::from(w), u64::from(h), u64::from(max));
    let fitted = if w >= h {
        (max64, (h64 * max64 / w64).max(1))
    } else {
        ((w64 * max64 / h64).max(1), max64)
    };
    // Both values are ≤ max, which came from a u32.
    (
        u32::try_from(fitted.0).unwrap_or(max),
        u32::try_from(fitted.1).unwrap_or(max),
    )
}

const BASE64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Standard base64 with padding (RFC 4648 §4).
#[cfg_attr(not(windows), allow(dead_code))]
pub fn base64(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk.first().copied().unwrap_or(0);
        let b1 = chunk.get(1).copied().unwrap_or(0);
        let b2 = chunk.get(2).copied().unwrap_or(0);
        let n = (u32::from(b0) << 16) | (u32::from(b1) << 8) | u32::from(b2);
        let sextet = |shift: u32| char::from(BASE64[((n >> shift) & 63) as usize]);
        out.push(sextet(18));
        out.push(sextet(12));
        out.push(if chunk.len() > 1 { sextet(6) } else { '=' });
        out.push(if chunk.len() > 2 { sextet(0) } else { '=' });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_matches_rfc_vectors() {
        let cases = [
            ("", ""),
            ("f", "Zg=="),
            ("fo", "Zm8="),
            ("foo", "Zm9v"),
            ("foob", "Zm9vYg=="),
            ("fooba", "Zm9vYmE="),
            ("foobar", "Zm9vYmFy"),
        ];
        for (input, expected) in cases {
            assert_eq!(base64(input.as_bytes()), expected);
        }
        assert_eq!(base64(&[0xff, 0xfe, 0xfd]), "//79");
    }

    #[test]
    fn converts_winrt_times() {
        assert_eq!(ticks_to_ms(1_234 * 10_000), 1_234);
        assert_eq!(ticks_to_ms(-5), 0);
        // 2026-11-14T00:00:00Z
        let unix_ms = 1_794_614_400_000_i64;
        assert_eq!(
            filetime_to_unix_ms(unix_ms * 10_000 + FILETIME_UNIX_EPOCH_TICKS),
            Some(unix_ms)
        );
        assert_eq!(filetime_to_unix_ms(0), None);
    }

    #[test]
    fn fits_covers_without_upscaling() {
        assert_eq!(fit_within(300, 300, 320), (300, 300));
        assert_eq!(fit_within(1280, 720, 320), (320, 180));
        assert_eq!(fit_within(500, 1000, 320), (160, 320));
        assert_eq!(fit_within(0, 10, 320), (0, 10));
    }

    #[test]
    fn track_id_changes_with_metadata() {
        let a = track_id("Spotify.exe", "Song", "Artist", "Album");
        assert_eq!(a, track_id("Spotify.exe", "Song", "Artist", "Album"));
        assert_ne!(a, track_id("Spotify.exe", "Other", "Artist", "Album"));
        assert_eq!(a.len(), 16);
    }

    #[test]
    fn commands_deserialize_from_lowercase() {
        let cmd: MediaCommand = serde_json::from_str("\"toggle\"").unwrap();
        assert_eq!(cmd, MediaCommand::Toggle);
        assert!(serde_json::from_str::<MediaCommand>("\"stop\"").is_err());
    }
}
