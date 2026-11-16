//! Steam downloads, read from the app manifests (`steamapps/appmanifest_*.acf`)
//! in every library listed in `steamapps/libraryfolders.vdf`.

use std::collections::HashMap;

use super::{Download, Source};

/// `EAppState` bits used here.
const FULLY_INSTALLED: u32 = 4;
const UPDATE_REQUIRED: u32 = 2;
const UPDATE_RUNNING: u32 = 256;
const UPDATE_PAUSED: u32 = 512;
const UPDATE_STARTED: u32 = 1024;
const DOWNLOADING: u32 = 1 << 20;
const STAGING: u32 = 1 << 21;
const SPEED_SMOOTHING: f64 = 0.4;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AppManifest {
    pub appid: u32,
    pub name: String,
    pub flags: u32,
    pub to_download: u64,
    pub downloaded: u64,
    pub to_stage: u64,
    pub staged: u64,
}

impl AppManifest {
    /// Steam is fetching or unpacking this app right now (not paused or queued).
    pub fn active(&self) -> bool {
        self.flags & UPDATE_PAUSED == 0
            && self.flags & (UPDATE_STARTED | UPDATE_RUNNING | DOWNLOADING | STAGING) != 0
    }

    pub fn ready(&self) -> bool {
        self.flags & FULLY_INSTALLED != 0 && self.flags & UPDATE_REQUIRED == 0 && !self.active()
    }

    /// Done and total bytes: downloading, then unpacking ("staging").
    pub fn progress(&self) -> (u64, u64) {
        (
            self.downloaded + self.staged,
            self.to_download + self.to_stage,
        )
    }
}

/// `"key"  "value"` pairs of a Valve KeyValues text file, in order (nesting ignored).
fn pairs(text: &str) -> impl Iterator<Item = (&str, String)> {
    text.lines().filter_map(|line| {
        let mut parts = line.split('"').skip(1).step_by(2);
        let key = parts.next()?;
        let value = parts.next()?;
        Some((key, value.replace("\\\\", "\\")))
    })
}

/// Reads an app manifest; the first occurrence of a key wins (the top level).
pub fn parse_manifest(text: &str) -> Option<AppManifest> {
    let mut found: HashMap<&str, String> = HashMap::new();
    for (key, value) in pairs(text) {
        found.entry(key).or_insert(value);
    }
    let num = |key: &str| {
        found
            .get(key)
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(0)
    };
    Some(AppManifest {
        appid: found.get("appid")?.parse().ok()?,
        name: found.get("name")?.clone(),
        flags: found.get("StateFlags")?.parse().ok()?,
        to_download: num("BytesToDownload"),
        downloaded: num("BytesDownloaded"),
        to_stage: num("BytesToStage"),
        staged: num("BytesStaged"),
    })
}

/// Library folders from `libraryfolders.vdf`.
pub fn library_paths(vdf: &str) -> Vec<String> {
    pairs(vdf)
        .filter(|(key, _)| *key == "path")
        .map(|(_, value)| value)
        .collect()
}

struct Track {
    done: u64,
    at: i64,
    speed: Option<f64>,
}

#[derive(Default)]
pub struct SteamTracker {
    active: HashMap<u32, Track>,
}

impl SteamTracker {
    /// Returns the apps downloading now and the names of those that just became ready.
    pub fn update(&mut self, now: i64, manifests: &[AppManifest]) -> (Vec<Download>, Vec<String>) {
        let mut finished = Vec::new();
        let mut active = HashMap::new();
        let mut list = Vec::new();
        for app in manifests {
            let previous = self.active.remove(&app.appid);
            if !app.active() {
                if previous.is_some() && app.ready() {
                    finished.push(app.name.clone());
                }
                continue;
            }
            let (done, total) = app.progress();
            let speed = match &previous {
                Some(prev) if done > prev.done && now > prev.at => {
                    let sample = (done - prev.done) as f64 / ((now - prev.at) as f64 / 1000.0);
                    Some(prev.speed.map_or(sample, |old| {
                        old * (1.0 - SPEED_SMOOTHING) + sample * SPEED_SMOOTHING
                    }))
                }
                Some(prev) => prev.speed,
                None => None,
            };
            // Steam rewrites the manifest every few seconds; keep the last change time.
            let at = previous
                .as_ref()
                .filter(|prev| prev.done == done)
                .map_or(now, |prev| prev.at);
            active.insert(app.appid, Track { done, at, speed });
            list.push(Download {
                id: format!("steam:{}", app.appid),
                source: Source::Steam,
                name: app.name.clone(),
                received: done,
                total: (total > 0).then_some(total),
                speed,
            });
        }
        self.active = active;
        list.sort_by(|a, b| a.id.cmp(&b.id));
        (list, finished)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MANIFEST: &str = r#""AppState"
{
	"appid"		"1422450"
	"name"		"Deadlock"
	"StateFlags"		"1026"
	"BytesToDownload"		"400"
	"BytesDownloaded"		"100"
	"BytesToStage"		"600"
	"BytesStaged"		"0"
	"InstalledDepots"
	{
		"1422456"
		{
			"manifest"		"123"
			"name"		"not the app name"
		}
	}
}"#;

    fn app(flags: u32, downloaded: u64) -> AppManifest {
        AppManifest {
            appid: 7,
            name: "Game".into(),
            flags,
            to_download: 1000,
            downloaded,
            ..Default::default()
        }
    }

    #[test]
    fn parses_manifests() {
        let m = parse_manifest(MANIFEST).unwrap();
        assert_eq!(m.appid, 1_422_450);
        assert_eq!(m.name, "Deadlock");
        assert!(m.active());
        assert_eq!(m.progress(), (100, 1000));
        assert_eq!(parse_manifest("\"AppState\"\n{\n}"), None);
    }

    #[test]
    fn reads_library_folders() {
        let vdf = "\"libraryfolders\"\n{\n\t\"0\"\n\t{\n\t\t\"path\"\t\t\"D:\\\\steam\"\n\t}\n\t\"1\"\n\t{\n\t\t\"path\"\t\t\"E:\\\\SteamLibrary\"\n\t}\n}";
        assert_eq!(library_paths(vdf), [r"D:\steam", r"E:\SteamLibrary"]);
    }

    #[test]
    fn paused_and_installed_apps_are_not_downloading() {
        // 1542: an update that was started and then paused.
        assert!(!app(1542, 0).active());
        assert!(!app(4, 0).active());
        assert!(app(4, 0).ready());
        assert!(!app(6, 0).ready());
    }

    #[test]
    fn tracks_progress_and_reports_ready_games() {
        let mut t = SteamTracker::default();
        let (list, _) = t.update(0, &[app(1026, 100)]);
        assert_eq!(list[0].total, Some(1000));
        assert_eq!(list[0].speed, None);
        let (list, _) = t.update(2000, &[app(1026, 300)]);
        assert_eq!(list[0].speed, Some(100.0));
        let (list, finished) = t.update(4000, &[app(4, 1000)]);
        assert!(list.is_empty());
        assert_eq!(finished, ["Game"]);
        // An app that was never seen downloading is not announced.
        assert!(t.update(6000, &[app(4, 1000)]).1.is_empty());
    }
}
