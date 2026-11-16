//! Browser downloads, seen as growing temporary files in the downloads folder:
//! Chromium browsers (Chrome, Edge, Yandex, Opera) write `name.crdownload`,
//! Firefox writes `name.part` (next to an empty `name` placeholder). Browsers
//! do not reveal the final size, so only bytes and speed are known.

use std::collections::HashMap;

use super::{Download, Source};

const SUFFIXES: [&str; 2] = [".crdownload", ".part"];
/// A temporary file that has not grown for this long is paused or abandoned.
const STALL_MS: i64 = 30_000;
/// Weight of the newest sample in the smoothed speed.
const SPEED_SMOOTHING: f64 = 0.4;

/// A temporary download file found in the folder.
pub struct Partial {
    pub file: String,
    pub size: u64,
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

struct Track {
    size: u64,
    at: i64,
    /// When the file last grew; `None` until it has been seen growing.
    grew_at: Option<i64>,
    speed: Option<f64>,
}

#[derive(Default)]
pub struct BrowserTracker {
    tracks: HashMap<String, Track>,
}

impl BrowserTracker {
    /// Takes a new look at the folder. Returns the downloads in progress and the
    /// final names of those that just finished (`finished_file` tells whether the
    /// final file exists with content).
    pub fn update(
        &mut self,
        now: i64,
        partials: Vec<Partial>,
        finished_file: impl Fn(&str) -> bool,
    ) -> (Vec<Download>, Vec<String>) {
        let mut seen = HashMap::with_capacity(partials.len());
        for Partial { file, size } in partials {
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
                        at: now,
                        grew_at: Some(now),
                        speed,
                    }
                }
                Some(prev) => Track {
                    size,
                    at: now,
                    ..prev
                },
                None => Track {
                    size,
                    at: now,
                    grew_at: None,
                    speed: None,
                },
            };
            seen.insert(file, track);
        }

        // Whatever vanished either finished (renamed to its final name) or was cancelled.
        let finished = self
            .tracks
            .iter()
            .filter(|(_, t)| t.grew_at.is_some())
            .filter_map(|(file, _)| final_name(file))
            .filter(|name| finished_file(name))
            .map(str::to_owned)
            .collect();
        self.tracks = seen;

        let mut active: Vec<Download> = self
            .tracks
            .iter()
            .filter(|(_, t)| t.grew_at.is_some_and(|at| now - at < STALL_MS))
            .map(|(file, t)| Download {
                id: format!("browser:{file}"),
                source: Source::Browser,
                name: final_name(file).unwrap_or(file).to_owned(),
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

    fn part(file: &str, size: u64) -> Partial {
        Partial {
            file: file.into(),
            size,
        }
    }

    #[test]
    fn recognises_temporary_files() {
        assert_eq!(final_name("report.pdf.crdownload"), Some("report.pdf"));
        assert_eq!(final_name("Setup.EXE.PART"), Some("Setup.EXE"));
        assert_eq!(final_name("photo.jpg"), None);
        assert_eq!(final_name(".part"), None);
    }

    #[test]
    fn shows_only_growing_files_with_speed() {
        let mut t = BrowserTracker::default();
        // A leftover from a crashed download that never grows stays hidden.
        let (active, _) = t.update(
            0,
            vec![part("old.zip.crdownload", 50), part("a.iso.crdownload", 0)],
            |_| false,
        );
        assert!(active.is_empty());
        let (active, _) = t.update(
            1000,
            vec![
                part("old.zip.crdownload", 50),
                part("a.iso.crdownload", 1_000_000),
            ],
            |_| false,
        );
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].name, "a.iso");
        assert_eq!(active[0].received, 1_000_000);
        assert_eq!(active[0].speed, Some(1_000_000.0));
        let (active, _) = t.update(2000, vec![part("a.iso.crdownload", 1_500_000)], |_| false);
        // Smoothed: 0.6 × 1 MB/s + 0.4 × 0.5 MB/s.
        assert!((active[0].speed.unwrap_or(0.0) - 800_000.0).abs() < 1.0);
    }

    #[test]
    fn hides_stalled_downloads() {
        let mut t = BrowserTracker::default();
        t.update(0, vec![part("a.iso.crdownload", 0)], |_| false);
        t.update(1000, vec![part("a.iso.crdownload", 10)], |_| false);
        let (active, _) = t.update(1000 + STALL_MS, vec![part("a.iso.crdownload", 10)], |_| {
            false
        });
        assert!(active.is_empty());
    }

    #[test]
    fn reports_finished_but_not_cancelled_downloads() {
        let mut t = BrowserTracker::default();
        t.update(
            0,
            vec![part("a.iso.crdownload", 0), part("b.zip.part", 0)],
            |_| false,
        );
        t.update(
            1000,
            vec![part("a.iso.crdownload", 5), part("b.zip.part", 5)],
            |_| false,
        );
        let (active, finished) = t.update(2000, Vec::new(), |name| name == "a.iso");
        assert!(active.is_empty());
        assert_eq!(finished, ["a.iso"]);
    }
}
