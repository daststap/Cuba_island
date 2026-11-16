//! Browser downloads, seen as temporary files being written in the downloads
//! folder: Chromium browsers (Chrome, Edge, Opera) write `name.crdownload`,
//! Yandex Browser writes `Не подтверждено 123456.~`, Firefox writes
//! `name.part` (next to an empty `name` placeholder). Browsers do not reveal
//! the final size, so only bytes and speed are known.
//!
//! A temporary file named "Unconfirmed 123456" / "Не подтверждено 123456" is
//! renamed to a different name at the end; the finished file is then found by
//! its creation time, which a rename keeps.
//!
//! A file counts as active while it grows *or* its modification time moves:
//! Chromium's parallel downloading writes several ranges at once, so the file
//! jumps close to its full length early and then fills in without growing.

use std::collections::HashMap;

use super::{Download, Source};

const SUFFIXES: [&str; 3] = [".crdownload", ".part", ".~"];
/// A temporary file that has not grown for this long is paused or abandoned.
const STALL_MS: i64 = 30_000;
/// Weight of the newest sample in the smoothed speed.
const SPEED_SMOOTHING: f64 = 0.4;

/// A temporary download file found in the folder.
pub struct Partial {
    pub file: String,
    pub size: u64,
    /// Last write, Unix ms.
    pub modified: i64,
    /// Creation, Unix ms (kept when the browser renames the finished file).
    pub created: i64,
}

/// `report.pdf.crdownload` → `report.pdf`; `None` for other files.
pub fn final_name(file: &str) -> Option<&str> {
    let lower = file.to_lowercase();
    SUFFIXES.iter().find_map(|suffix| {
        lower
            .ends_with(suffix)
            .then(|| &file[..file.len() - suffix.len()])
            .filter(|name| !name.is_empty())
    })
}

/// "Unconfirmed 123456" / "Не подтверждено 942813": the real name comes later.
pub fn is_unconfirmed(name: &str) -> bool {
    !name.contains('.')
        && name
            .rsplit_once(' ')
            .is_some_and(|(_, n)| n.len() >= 4 && n.chars().all(|c| c.is_ascii_digit()))
}

struct Track {
    size: u64,
    created: i64,
    at: i64,
    modified: i64,
    /// When the file was last written to; `None` while it looks abandoned.
    active_at: Option<i64>,
    speed: Option<f64>,
}

#[derive(Default)]
pub struct BrowserTracker {
    tracks: HashMap<String, Track>,
}

impl BrowserTracker {
    /// Takes a new look at the folder. Returns the downloads in progress and the
    /// names of those that just finished: `resolve(temporary file, created)`
    /// finds the finished file, or `None` if the download was cancelled.
    pub fn update(
        &mut self,
        now: i64,
        partials: Vec<Partial>,
        resolve: impl Fn(&str, i64) -> Option<String>,
    ) -> (Vec<Download>, Vec<String>) {
        let mut seen = HashMap::with_capacity(partials.len());
        for Partial {
            file,
            size,
            modified,
            created,
        } in partials
        {
            let track = match self.tracks.remove(&file) {
                Some(prev) if size > prev.size => {
                    let seconds = (now - prev.at) as f64 / 1000.0;
                    let sample = (seconds > 0.0).then(|| (size - prev.size) as f64 / seconds);
                    let speed = match (prev.speed, sample) {
                        (Some(old), Some(new)) => {
                            Some(old * (1.0 - SPEED_SMOOTHING) + new * SPEED_SMOOTHING)
                        }
                        (old, new) => new.or(old),
                    };
                    Track {
                        size,
                        created,
                        at: now,
                        modified,
                        active_at: Some(now),
                        speed,
                    }
                }
                // Written to without growing (parallel ranges filling in).
                Some(prev) if modified != prev.modified => Track {
                    size,
                    created,
                    at: now,
                    modified,
                    active_at: Some(now),
                    speed: prev.speed,
                },
                Some(prev) => Track {
                    size,
                    at: now,
                    ..prev
                },
                // First sight: active if it was written to just now (a download
                // already running when the island started); old leftovers are not.
                None => Track {
                    size,
                    created,
                    at: now,
                    modified,
                    active_at: (now - modified < STALL_MS).then_some(modified),
                    speed: None,
                },
            };
            seen.insert(file, track);
        }

        // Whatever vanished either finished (renamed to its final name) or was cancelled.
        let finished = self
            .tracks
            .iter()
            .filter(|(_, t)| t.active_at.is_some())
            .filter_map(|(file, t)| resolve(file, t.created))
            .collect();
        self.tracks = seen;

        let mut active: Vec<Download> = self
            .tracks
            .iter()
            .filter(|(_, t)| t.active_at.is_some_and(|at| now - at < STALL_MS))
            .map(|(file, t)| Download {
                id: format!("browser:{file}"),
                source: Source::Browser,
                // Empty while the browser has not given the real name yet.
                name: final_name(file)
                    .filter(|name| !is_unconfirmed(name))
                    .unwrap_or_default()
                    .to_owned(),
                received: t.size,
                total: None,
                speed: t.speed,
            })
            .collect();
        active.sort_by(|a, b| a.id.cmp(&b.id));
        (active, finished)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A file whose modification time moves with its size (a plain download).
    fn part(file: &str, size: u64) -> Partial {
        written(file, size, -(STALL_MS * 10) + size as i64)
    }

    fn written(file: &str, size: u64, modified: i64) -> Partial {
        Partial {
            file: file.into(),
            size,
            modified,
            created: 42,
        }
    }

    /// Finishes only `name`, found by stripping the temporary suffix.
    fn finishes(name: &'static str) -> impl Fn(&str, i64) -> Option<String> {
        move |temp, _| final_name(temp).filter(|n| *n == name).map(String::from)
    }

    fn nothing(_: &str, _: i64) -> Option<String> {
        None
    }

    #[test]
    fn recognises_temporary_files() {
        assert_eq!(final_name("report.pdf.crdownload"), Some("report.pdf"));
        assert_eq!(final_name("Setup.EXE.PART"), Some("Setup.EXE"));
        assert_eq!(final_name("photo.jpg"), None);
        assert_eq!(final_name(".part"), None);
        assert_eq!(
            final_name("Не подтверждено 942813.~"),
            Some("Не подтверждено 942813")
        );
        assert!(is_unconfirmed("Не подтверждено 942813"));
        assert!(is_unconfirmed("Unconfirmed 123456"));
        assert!(!is_unconfirmed("Report 2024.pdf"));
        assert!(!is_unconfirmed("Season 2"));
    }

    #[test]
    fn finds_yandex_downloads_renamed_at_the_end() {
        let mut t = BrowserTracker::default();
        let temp = "Не подтверждено 942813.~";
        t.update(0, vec![written(temp, 1000, -1)], nothing);
        let (active, _) = t.update(1000, vec![written(temp, 5000, 900)], nothing);
        assert_eq!(active[0].name, "");
        assert_eq!(active[0].received, 5000);
        let (_, finished) = t.update(2000, Vec::new(), |file, created| {
            (file == temp && created == 42).then(|| "Game.zip".to_owned())
        });
        assert_eq!(finished, ["Game.zip"]);
    }

    #[test]
    fn shows_only_growing_files_with_speed() {
        let mut t = BrowserTracker::default();
        // A leftover from a crashed download that never grows stays hidden.
        let (active, _) = t.update(
            0,
            vec![part("old.zip.crdownload", 50), part("a.iso.crdownload", 0)],
            nothing,
        );
        assert!(active.is_empty());
        let (active, _) = t.update(
            1000,
            vec![
                part("old.zip.crdownload", 50),
                part("a.iso.crdownload", 1_000_000),
            ],
            nothing,
        );
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].name, "a.iso");
        assert_eq!(active[0].received, 1_000_000);
        assert_eq!(active[0].speed, Some(1_000_000.0));
        let (active, _) = t.update(2000, vec![part("a.iso.crdownload", 1_500_000)], nothing);
        // Smoothed: 0.6 × 1 MB/s + 0.4 × 0.5 MB/s.
        assert!((active[0].speed.unwrap_or(0.0) - 800_000.0).abs() < 1.0);
    }

    #[test]
    fn hides_stalled_downloads() {
        let mut t = BrowserTracker::default();
        t.update(0, vec![part("a.iso.crdownload", 0)], nothing);
        t.update(1000, vec![part("a.iso.crdownload", 10)], nothing);
        let (active, _) = t.update(1000 + STALL_MS, vec![part("a.iso.crdownload", 10)], nothing);
        assert!(active.is_empty());
    }

    #[test]
    fn reports_finished_but_not_cancelled_downloads() {
        let mut t = BrowserTracker::default();
        t.update(
            0,
            vec![part("a.iso.crdownload", 0), part("b.zip.part", 0)],
            nothing,
        );
        t.update(
            1000,
            vec![part("a.iso.crdownload", 5), part("b.zip.part", 5)],
            nothing,
        );
        let (active, finished) = t.update(2000, Vec::new(), finishes("a.iso"));
        assert!(active.is_empty());
        assert_eq!(finished, ["a.iso"]);
    }

    #[test]
    fn follows_parallel_downloads_that_do_not_grow() {
        let mut t = BrowserTracker::default();
        // Already near full length on first sight, written a moment ago.
        let (active, _) = t.update(
            10_000,
            vec![written("big.zip.crdownload", 900, 9_500)],
            nothing,
        );
        assert_eq!(active.len(), 1);
        let (active, _) = t.update(
            11_000,
            vec![written("big.zip.crdownload", 900, 10_900)],
            nothing,
        );
        assert_eq!(active[0].received, 900);
        let (_, finished) = t.update(12_000, Vec::new(), finishes("big.zip"));
        assert_eq!(finished, ["big.zip"]);
    }
}
