//! Command line: find and launch apps, calculate, search the web (PLAN.md stage 4).

mod calc;
#[cfg(windows)]
mod index;
mod search;

use std::collections::HashMap;
use std::sync::{Arc, PoisonError, RwLock};

use rusqlite::params;
use serde::Serialize;
use tauri::{AppHandle, Manager, State};

use crate::error::{AppError, AppResult};
use crate::storage::Db;
use search::Candidate;

/// At most this many rows, including the calculator and web-search rows (PLAN.md).
const MAX_RESULTS: usize = 8;
const WEB_SEARCH_URL: &str = "https://ya.ru/search/";

/// A launchable app as kept in memory.
#[derive(Clone, Debug)]
struct Entry {
    id: String,
    name: String,
    icon: Option<String>,
}

/// AUMID of the Windows Settings app (called «Настройки» or «Параметры» depending on the build).
pub(crate) const SETTINGS_AUMID: &str =
    "windows.immersivecontrolpanel_cw5n1h2txyewy!microsoft.windows.immersivecontrolpanel";

/// Extra words some well-known apps should be found by.
fn app_keywords(id: &str) -> &'static str {
    match id.strip_prefix("app:") {
        Some(SETTINGS_AUMID) => "Параметры Настройки Settings",
        _ => "",
    }
}

/// Built-in commands: id and words they are found by (the label lives in `ru.ts`).
const COMMANDS: &[(&str, &str)] = &[
    ("cmd:settings", "Настройки Escape settings"),
    (
        "cmd:screenshot",
        "Скриншот области экрана снимок screenshot",
    ),
    ("cmd:lock", "Заблокировать компьютер lock"),
    ("cmd:quit", "Выйти из Escape quit exit"),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ResultKind {
    App,
    Command,
    Calc,
    Web,
}

/// One row of results. Mirrored in `src/ipc/commands.ts`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchResult {
    /// What `launcher_run` gets: `app:…`, `cmd:…`, `calc:…` or `web:…`.
    pub id: String,
    pub kind: ResultKind,
    /// App name, calculator answer or the search query; commands are labelled on the frontend.
    pub title: String,
    pub icon_path: Option<String>,
}

pub struct LauncherShared {
    apps: RwLock<Arc<Vec<Entry>>>,
    db: Db,
}

impl LauncherShared {
    fn apps(&self) -> Arc<Vec<Entry>> {
        self.apps
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    fn launch_counts(&self) -> HashMap<String, u32> {
        let db = self.db.lock();
        let rows = db
            .prepare("SELECT id, count FROM launches")
            .and_then(|mut stmt| {
                stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
                    .collect::<rusqlite::Result<HashMap<_, _>>>()
            });
        rows.unwrap_or_else(|err| {
            log::warn!("failed to read launch counts: {err}");
            HashMap::new()
        })
    }

    fn record_launch(&self, id: &str) {
        let result = self.db.lock().execute(
            "INSERT INTO launches (id, count, last_at) VALUES (?1, 1, ?2)
             ON CONFLICT(id) DO UPDATE SET count = count + 1, last_at = excluded.last_at",
            params![id, crate::util::now_ms()],
        );
        if let Err(err) = result {
            log::warn!("failed to record launch of {id}: {err}");
        }
    }
}

/// Builds the result list for a query (pure apart from the inputs; tested).
fn build_results(
    query: &str,
    apps: &[Entry],
    launches: &HashMap<String, u32>,
) -> Vec<LaunchResult> {
    let query = query.trim();
    if query.is_empty() {
        return Vec::new();
    }
    let mut results = Vec::new();
    if let Some(value) = calc::evaluate(query) {
        let answer = calc::format_number(value);
        results.push(LaunchResult {
            id: format!("calc:{answer}"),
            kind: ResultKind::Calc,
            title: answer,
            icon_path: None,
        });
    }

    let candidates = apps
        .iter()
        .map(|app| Candidate {
            id: &app.id,
            name: &app.name,
            keywords: app_keywords(&app.id),
        })
        .chain(COMMANDS.iter().map(|(id, words)| Candidate {
            id,
            name: words,
            keywords: "",
        }));
    let room = MAX_RESULTS - results.len() - 1; // keep the last row for web search
    for id in search::rank(query, candidates, launches, room) {
        if let Some(app) = apps.iter().find(|app| app.id == id) {
            results.push(LaunchResult {
                id: app.id.clone(),
                kind: ResultKind::App,
                title: app.name.clone(),
                icon_path: app.icon.clone(),
            });
        } else {
            results.push(LaunchResult {
                id: id.to_owned(),
                kind: ResultKind::Command,
                title: String::new(),
                icon_path: None,
            });
        }
    }

    results.push(LaunchResult {
        id: format!("web:{query}"),
        kind: ResultKind::Web,
        title: query.to_owned(),
        icon_path: None,
    });
    results
}

/// Indexes apps now and every 10 minutes in the background.
pub fn start(app: &AppHandle, db: Db) -> AppResult<()> {
    let shared = Arc::new(LauncherShared {
        apps: RwLock::new(Arc::new(Vec::new())),
        db,
    });
    app.manage(shared.clone());

    #[cfg(windows)]
    {
        use windows::Win32::System::Com::{CoInitializeEx, COINIT_APARTMENTTHREADED};

        const REINDEX_EVERY: std::time::Duration = std::time::Duration::from_secs(10 * 60);
        let icon_dir = app.path().app_cache_dir()?.join("icons");
        crate::thread::spawn_supervised("launcher-index", move || {
            // SAFETY: initialises COM (STA, as shell objects expect) for this thread only.
            let _ = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
            loop {
                let started = std::time::Instant::now();
                match index::scan(&icon_dir) {
                    Ok(found) => {
                        log::info!(
                            "indexed {} apps in {} ms",
                            found.len(),
                            started.elapsed().as_millis()
                        );
                        let entries = found
                            .into_iter()
                            .map(|app| Entry {
                                id: format!("app:{}", app.parsing_name),
                                name: app.name,
                                icon: app.icon.map(|p| p.display().to_string()),
                            })
                            .collect();
                        *shared.apps.write().unwrap_or_else(PoisonError::into_inner) =
                            Arc::new(entries);
                    }
                    Err(err) => log::error!("failed to index apps: {err}"),
                }
                std::thread::sleep(REINDEX_EVERY);
            }
        });
    }
    Ok(())
}

#[tauri::command]
pub fn launcher_search(
    query: String,
    shared: State<'_, Arc<LauncherShared>>,
) -> AppResult<Vec<LaunchResult>> {
    let started = std::time::Instant::now();
    let results = build_results(&query, &shared.apps(), &shared.launch_counts());
    log::debug!(
        "launcher_search({} chars): {} results in {} µs",
        query.chars().count(),
        results.len(),
        started.elapsed().as_micros()
    );
    Ok(results)
}

/// Runs a result. Apps, web search and lock hand focus over to what opens next;
/// the calculator copies its answer (the frontend then closes the island).
#[tauri::command]
pub async fn launcher_run(
    id: String,
    app: AppHandle,
    shared: State<'_, Arc<LauncherShared>>,
) -> AppResult<()> {
    let (kind, value) = id
        .split_once(':')
        .ok_or_else(|| AppError::InvalidArgument(format!("неизвестный результат {id}")))?;
    match kind {
        "calc" => {
            arboard::Clipboard::new()
                .and_then(|mut clipboard| clipboard.set_text(value))
                .map_err(|err| AppError::Internal(format!("не удалось скопировать: {err}")))?;
        }
        "web" => {
            let url = tauri::Url::parse_with_params(WEB_SEARCH_URL, [("text", value)])
                .map_err(|err| AppError::Internal(err.to_string()))?;
            crate::window::hand_over_focus(&app)?;
            use tauri_plugin_opener::OpenerExt;
            app.opener()
                .open_url(url.as_str(), None::<&str>)
                .map_err(|err| AppError::Internal(err.to_string()))?;
        }
        "app" => {
            crate::window::hand_over_focus(&app)?;
            std::process::Command::new("explorer.exe")
                .arg(format!("shell:AppsFolder\\{value}"))
                .spawn()?;
            shared.record_launch(&id);
        }
        "cmd" => {
            shared.record_launch(&id);
            match value {
                "quit" => app.exit(0),
                // The island folds away first, then the screen is captured.
                "screenshot" => {
                    // The overlay takes focus itself, like an app being opened.
                    crate::window::hand_over_focus(&app)?;
                    crate::screenshot::start(&app, crate::screenshot::COLLAPSE_DELAY);
                }
                "lock" => {
                    crate::window::hand_over_focus(&app)?;
                    #[cfg(windows)]
                    // SAFETY: plain call; it only requests the lock screen.
                    unsafe { windows::Win32::System::Shutdown::LockWorkStation() }?;
                }
                // Settings open inside the island; the frontend handles it.
                "settings" => {}
                _ => {
                    return Err(AppError::InvalidArgument(format!(
                        "неизвестная команда {id}"
                    )))
                }
            }
        }
        _ => {
            return Err(AppError::InvalidArgument(format!(
                "неизвестный результат {id}"
            )))
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn apps() -> Vec<Entry> {
        [
            "Google Chrome",
            "Telegram Desktop",
            "Калькулятор",
            "Параметры",
        ]
        .iter()
        .map(|name| Entry {
            id: format!("app:{name}"),
            name: (*name).to_owned(),
            icon: None,
        })
        .collect()
    }

    #[test]
    fn calculator_first_and_web_search_last() {
        let results = build_results("2+2*2", &apps(), &HashMap::new());
        assert_eq!(results[0].kind, ResultKind::Calc);
        assert_eq!(results[0].title, "6");
        assert_eq!(results[0].id, "calc:6");
        let last = results.last().unwrap();
        assert_eq!((last.kind, last.title.as_str()), (ResultKind::Web, "2+2*2"));
    }

    #[test]
    fn finds_apps_and_commands() {
        let results = build_results("кальк", &apps(), &HashMap::new());
        assert_eq!(results[0].title, "Калькулятор");
        let lock = build_results("lock", &apps(), &HashMap::new());
        assert_eq!(lock[0].id, "cmd:lock");
        assert_eq!(lock[0].kind, ResultKind::Command);
    }

    #[test]
    fn finds_windows_settings_by_either_name() {
        let settings = vec![Entry {
            id: format!("app:{SETTINGS_AUMID}"),
            name: "Настройки".to_owned(),
            icon: None,
        }];
        for query in ["парам", "настр", "settings"] {
            let results = build_results(query, &settings, &HashMap::new());
            assert_eq!(results[0].title, "Настройки", "query {query}");
        }
    }

    #[test]
    fn never_exceeds_eight_rows() {
        let many: Vec<Entry> = (0..30)
            .map(|i| Entry {
                id: format!("app:a{i}"),
                name: format!("App {i}"),
                icon: None,
            })
            .collect();
        assert_eq!(
            build_results("app", &many, &HashMap::new()).len(),
            MAX_RESULTS
        );
    }

    #[test]
    fn empty_query_shows_nothing() {
        assert!(build_results("  ", &apps(), &HashMap::new()).is_empty());
    }
}
