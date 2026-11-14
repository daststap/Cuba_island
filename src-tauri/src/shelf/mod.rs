//! File shelf (PLAN.md stage 6): drop files on the island to park them there,
//! then drag them on to another app. The shelf keeps references, not copies.

mod store;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::error::{AppError, AppResult};
use crate::storage::Db;
use crate::util::now_ms;

/// Previews are shown at 48 logical px; 96 keeps them sharp at 200%.
#[cfg_attr(not(windows), allow(dead_code))]
const IMAGE_SIZE: i32 = 96;

pub struct ShelfShared {
    db: Db,
    /// `app_cache_dir/shelf`: icons and thumbnails.
    image_dir: PathBuf,
}

type Shared = Arc<ShelfShared>;

/// One shelf item. Mirrored in `src/ipc/commands.ts`.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShelfItem {
    pub id: i64,
    pub path: String,
    pub name: String,
    pub is_dir: bool,
    /// Bytes; `None` for folders and missing files.
    pub size: Option<u64>,
    /// The original was moved or deleted.
    pub missing: bool,
    /// PNG icon or thumbnail, served through the asset protocol.
    pub image_path: Option<String>,
    pub added_at: i64,
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

/// Cache key: path + modification time, so an edited picture gets a fresh thumbnail.
#[cfg_attr(not(windows), allow(dead_code))]
fn image_cache_path(dir: &Path, path: &Path, modified: Option<std::time::SystemTime>) -> PathBuf {
    let stamp = modified
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_secs());
    let key = format!("{}|{stamp}", path.display());
    dir.join(format!("{:016x}.png", crate::util::fnv1a(key.as_bytes())))
}

#[cfg(windows)]
fn image_for(dir: &Path, path: &Path, modified: Option<std::time::SystemTime>) -> Option<String> {
    let cached = image_cache_path(dir, path, modified);
    if !cached.exists() {
        let _ = std::fs::create_dir_all(dir);
        let item = crate::shell::item_from_path(path).ok()?;
        if let Err(err) = crate::shell::save_image(&item, &cached, IMAGE_SIZE, false) {
            log::debug!("no shelf image for {}: {err}", path.display());
            return None;
        }
    }
    Some(cached.display().to_string())
}

#[cfg(not(windows))]
fn image_for(_: &Path, _: &Path, _: Option<std::time::SystemTime>) -> Option<String> {
    None
}

impl ShelfShared {
    /// Builds the UI list; reads file metadata and makes previews (call off the main thread).
    fn items(&self) -> AppResult<Vec<ShelfItem>> {
        let entries = store::list(&self.db.lock())?;
        Ok(entries
            .into_iter()
            .map(|entry| {
                let path = PathBuf::from(&entry.path);
                let meta = std::fs::metadata(&path).ok();
                let is_dir = meta.as_ref().is_some_and(std::fs::Metadata::is_dir);
                ShelfItem {
                    id: entry.id,
                    name: file_name(&path),
                    is_dir,
                    size: meta
                        .as_ref()
                        .filter(|m| m.is_file())
                        .map(std::fs::Metadata::len),
                    missing: meta.is_none(),
                    image_path: meta
                        .as_ref()
                        .and_then(|m| image_for(&self.image_dir, &path, m.modified().ok())),
                    path: entry.path,
                    added_at: entry.added_at,
                }
            })
            .collect())
    }
}

fn changed(app: &AppHandle) {
    if let Err(err) = app.emit("shelf://changed", ()) {
        log::warn!("failed to emit shelf://changed: {err}");
    }
}

pub fn start(app: &AppHandle, db: Db) -> AppResult<()> {
    let shared: Shared = Arc::new(ShelfShared {
        db,
        image_dir: app.path().app_cache_dir()?.join("shelf"),
    });
    app.manage(shared.clone());

    // The island window is transparent and has no child webview HWND, so Windows
    // delivers file drops to the window itself. Tauri reports them as a *window*
    // event (and in some versions a webview event); listen to both here, in Rust,
    // instead of relying on which one the frontend API subscribes to.
    let window = app
        .get_webview_window(crate::window::WINDOW_LABEL)
        .ok_or_else(|| AppError::Internal("нет окна острова".into()))?;
    let (app_w, shared_w) = (app.clone(), shared.clone());
    window.on_window_event(move |event| {
        if let tauri::WindowEvent::DragDrop(drag) = event {
            on_drag(&app_w, &shared_w, drag);
        }
    });
    let app_v = app.clone();
    window.on_webview_event(move |event| {
        if let tauri::WebviewEvent::DragDrop(drag) = event {
            on_drag(&app_v, &shared, drag);
        }
    });
    Ok(())
}

/// Payload of `shelf://drag`. Mirrored in `src/ipc/events.ts`.
#[derive(Clone, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
enum DragPayload {
    Enter,
    Leave,
    Drop,
}

fn on_drag(app: &AppHandle, shared: &ShelfShared, drag: &tauri::DragDropEvent) {
    let payload = match drag {
        tauri::DragDropEvent::Enter { .. } => DragPayload::Enter,
        tauri::DragDropEvent::Drop { paths, .. } => {
            let paths: Vec<String> = paths
                .iter()
                .filter(|p| p.is_absolute())
                .map(|p| p.display().to_string())
                .collect();
            log::debug!("{} file(s) dropped on the island", paths.len());
            if !paths.is_empty() {
                match store::add(&shared.db.lock(), &paths, now_ms()) {
                    Ok(()) => changed(app),
                    Err(err) => log::error!("failed to put dropped files on the shelf: {err}"),
                }
            }
            DragPayload::Drop
        }
        tauri::DragDropEvent::Leave => DragPayload::Leave,
        _ => return,
    };
    if let Err(err) = app.emit("shelf://drag", payload) {
        log::warn!("failed to emit shelf://drag: {err}");
    }
}

/// Runs `work` on a blocking thread with COM initialised (shell images need it).
async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> AppResult<T> + Send + 'static,
) -> AppResult<T> {
    tauri::async_runtime::spawn_blocking(move || {
        #[cfg(windows)]
        {
            use windows::Win32::System::Com::{CoInitializeEx, COINIT_APARTMENTTHREADED};
            // SAFETY: initialises COM for this pool thread; repeated calls are harmless.
            let _ = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
        }
        work()
    })
    .await
    .map_err(|err| AppError::Internal(err.to_string()))?
}

#[tauri::command]
pub async fn shelf_list(shared: State<'_, Shared>) -> AppResult<Vec<ShelfItem>> {
    let shared = shared.inner().clone();
    blocking(move || shared.items()).await
}

#[tauri::command]
pub fn shelf_remove(ids: Vec<i64>, app: AppHandle, shared: State<'_, Shared>) -> AppResult<()> {
    store::remove(&shared.db.lock(), &ids)?;
    changed(&app);
    Ok(())
}

#[tauri::command]
pub fn shelf_clear(app: AppHandle, shared: State<'_, Shared>) -> AppResult<()> {
    store::clear(&shared.db.lock())?;
    changed(&app);
    Ok(())
}

/// Opens a shelf item with its default app.
#[tauri::command]
pub fn shelf_open(id: i64, app: AppHandle, shared: State<'_, Shared>) -> AppResult<()> {
    let entry = store::get(&shared.db.lock(), &[id])?
        .pop()
        .ok_or_else(|| AppError::InvalidArgument("этого файла нет на полке".into()))?;
    use tauri_plugin_opener::OpenerExt;
    app.opener()
        .open_path(&entry.path, None::<&str>)
        .map_err(|err| AppError::Unavailable(format!("не удалось открыть файл: {err}")))
}

/// Starts dragging items out to another app (call on mouse-down + move). Items that
/// were dropped somewhere leave the shelf. Returns whether they were dropped.
#[tauri::command]
pub async fn shelf_drag_out(
    ids: Vec<i64>,
    app: AppHandle,
    shared: State<'_, Shared>,
) -> AppResult<bool> {
    let entries = store::get(&shared.db.lock(), &ids)?;
    let paths: Vec<PathBuf> = entries
        .iter()
        .map(|e| PathBuf::from(&e.path))
        .filter(|p| p.exists())
        .collect();
    if paths.is_empty() {
        return Err(AppError::Unavailable("файл не найден на диске".into()));
    }

    #[cfg(windows)]
    let dropped = {
        let window = app
            .get_webview_window(crate::window::WINDOW_LABEL)
            .ok_or_else(|| AppError::Internal("нет окна острова".into()))?;
        let hwnd = window.hwnd()?;
        let hwnd_raw = hwnd.0 as isize;
        let (tx, rx) = std::sync::mpsc::channel();
        // OLE drag must run on the window's thread (the main thread); it blocks there
        // in a modal loop until the user drops or cancels.
        app.run_on_main_thread(move || {
            let hwnd = windows::Win32::Foundation::HWND(hwnd_raw as *mut std::ffi::c_void);
            let result = crate::shell::drag_files(hwnd, &paths).map_err(|err| err.to_string());
            let _ = tx.send(result);
        })?;
        tauri::async_runtime::spawn_blocking(move || rx.recv())
            .await
            .map_err(|err| AppError::Internal(err.to_string()))?
            .map_err(|_| AppError::Internal("перетаскивание прервано".into()))?
            .map_err(|err| AppError::Internal(format!("не удалось перетащить: {err}")))?
    };
    #[cfg(not(windows))]
    let dropped = {
        let _ = paths;
        false
    };

    if dropped {
        let ids: Vec<i64> = entries.iter().map(|e| e.id).collect();
        store::remove(&shared.db.lock(), &ids)?;
        changed(&app);
    }
    Ok(dropped)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_files_and_folders() {
        assert_eq!(file_name(Path::new("C:\\docs\\отчёт.pdf")), "отчёт.pdf");
        assert_eq!(file_name(Path::new("D:\\projects\\escape")), "escape");
    }

    #[test]
    fn preview_cache_changes_with_modification_time() {
        let dir = Path::new("cache");
        let t1 = std::time::UNIX_EPOCH + std::time::Duration::from_secs(100);
        let t2 = std::time::UNIX_EPOCH + std::time::Duration::from_secs(200);
        let a = image_cache_path(dir, Path::new("C:\\a.png"), Some(t1));
        assert_eq!(a, image_cache_path(dir, Path::new("C:\\a.png"), Some(t1)));
        assert_ne!(a, image_cache_path(dir, Path::new("C:\\a.png"), Some(t2)));
        assert_ne!(a, image_cache_path(dir, Path::new("C:\\b.png"), Some(t1)));
    }
}
