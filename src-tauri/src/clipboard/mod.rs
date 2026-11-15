//! Clipboard history: listener, storage and paste (PLAN.md stage 3).

mod images;
mod store;
#[cfg(windows)]
mod watcher;
#[cfg(windows)]
mod win32;

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, MutexGuard};

use rusqlite::Connection;
use tauri::{AppHandle, Manager, State};

use crate::error::{AppError, AppResult};
use crate::storage::Db;
use store::{Clip, ClipItem, ClipKind, NewClip};

pub struct ClipboardShared {
    db: Db,
    /// `app_data_dir/clips`: PNGs and their previews.
    dir: PathBuf,
    /// History size from settings; pinned clips do not count.
    limit: AtomicUsize,
}

impl ClipboardShared {
    fn db(&self) -> MutexGuard<'_, Connection> {
        self.db.lock()
    }

    /// Deletes the oldest unpinned clips beyond the limit, with their image files.
    fn trim(&self) -> AppResult<()> {
        let removed = store::enforce_limit(&self.db(), self.limit.load(Ordering::SeqCst))?;
        for name in removed {
            images::remove(&self.dir, &name);
        }
        Ok(())
    }

    fn item(&self, clip: &Clip) -> ClipItem {
        let thumb = clip.image_path.as_ref().map(|name| {
            self.dir
                .join(images::thumb_name(name))
                .display()
                .to_string()
        });
        clip.to_item(thumb)
    }

    /// Stores a new clip (or bumps a duplicate) and trims the history.
    #[cfg_attr(not(windows), allow(dead_code))]
    fn insert(&self, clip: NewClip<'_>, hash: &str) -> AppResult<ClipItem> {
        let (clip, _is_new) = store::upsert(&self.db(), clip, hash, now_ms())?;
        self.trim()?;
        Ok(self.item(&clip))
    }

    /// Stores an RGBA image (saving its PNG once per content) as a new clip.
    fn insert_image(&self, width: u32, height: u32, rgba: Vec<u8>) -> AppResult<ClipItem> {
        let hash = store::content_hash(ClipKind::Image, &rgba);
        let stem = hash.rsplit(':').next().unwrap_or("image").to_owned();
        let name = format!("{stem}.png");
        if !self.dir.join(&name).exists() {
            images::save(&self.dir, &stem, width, height, rgba)?;
        }
        self.insert(NewClip::Image { path: &name }, &hash)
    }

    fn get(&self, id: i64) -> AppResult<Clip> {
        store::get(&self.db(), id)?
            .ok_or_else(|| AppError::InvalidArgument(format!("записи {id} нет в истории")))
    }

    /// Puts a clip on the system clipboard and moves it to the top of the history.
    fn write_to_clipboard(&self, id: i64) -> AppResult<()> {
        let clip = self.get(id)?;
        let mut clipboard =
            arboard::Clipboard::new().map_err(|err| AppError::Internal(err.to_string()))?;
        let result = match (clip.kind, &clip.text, &clip.image_path) {
            (ClipKind::Text, Some(text), _) => clipboard.set_text(text.as_str()),
            (ClipKind::Image, _, Some(name)) => {
                let (width, height, bytes) = images::load(&self.dir, name)?;
                clipboard.set_image(arboard::ImageData {
                    width: width as usize,
                    height: height as usize,
                    bytes: bytes.into(),
                })
            }
            _ => return Err(AppError::Internal(format!("запись {id} повреждена"))),
        };
        result.map_err(|err| AppError::Internal(format!("не удалось записать в буфер: {err}")))?;
        store::touch(&self.db(), id, now_ms())
    }
}

type Shared = Arc<ClipboardShared>;

use crate::util::now_ms;

/// Changes the history size and trims the history right away.
pub fn set_history_limit(app: &AppHandle, limit: usize) -> AppResult<()> {
    let shared = app.state::<Shared>();
    shared.limit.store(limit, Ordering::SeqCst);
    shared.trim()
}

/// Starts the clipboard listener on top of the shared database.
pub fn start(app: &AppHandle, db: Db, limit: usize) -> AppResult<()> {
    let shared = Arc::new(ClipboardShared {
        db,
        dir: app.path().app_data_dir()?.join("clips"),
        limit: AtomicUsize::new(limit),
    });
    app.manage(shared.clone());

    #[cfg(windows)]
    {
        use tauri::Emitter;

        let app = app.clone();
        crate::thread::spawn_supervised("clipboard", move || {
            let emit = |item: ClipItem| {
                if let Err(err) = app.emit("clipboard://added", item) {
                    log::warn!("failed to emit clipboard://added: {err}");
                }
            };
            if let Err(err) = watcher::run(shared.clone(), emit) {
                log::error!("clipboard listener stopped: {err}");
            }
        });
    }
    Ok(())
}

/// Puts an RGBA image on the system clipboard and into the history (the listener
/// skips our own writes), announcing it like any other copy.
pub fn copy_image(app: &AppHandle, width: u32, height: u32, rgba: Vec<u8>) -> AppResult<()> {
    use tauri::Emitter;

    let mut clipboard =
        arboard::Clipboard::new().map_err(|err| AppError::Internal(err.to_string()))?;
    clipboard
        .set_image(arboard::ImageData {
            width: width as usize,
            height: height as usize,
            bytes: (&rgba[..]).into(),
        })
        .map_err(|err| AppError::Internal(format!("не удалось записать в буфер: {err}")))?;
    let item = app.state::<Shared>().insert_image(width, height, rgba)?;
    if let Err(err) = app.emit("clipboard://added", item) {
        log::warn!("failed to emit clipboard://added: {err}");
    }
    Ok(())
}

#[tauri::command]
pub fn clipboard_list(
    query: Option<String>,
    limit: usize,
    offset: usize,
    shared: State<'_, Shared>,
) -> AppResult<Vec<ClipItem>> {
    let clips = store::list(&shared.db(), query.as_deref(), limit.min(500), offset)?;
    Ok(clips.iter().map(|clip| shared.item(clip)).collect())
}

/// Copies a clip to the clipboard without pasting.
#[tauri::command]
pub async fn clipboard_copy(id: i64, shared: State<'_, Shared>) -> AppResult<()> {
    shared.write_to_clipboard(id)
}

/// Copies a clip and pastes it into the window the user came from (PLAN.md stage 3).
#[tauri::command]
pub async fn clipboard_paste(id: i64, app: AppHandle, shared: State<'_, Shared>) -> AppResult<()> {
    shared.write_to_clipboard(id)?;
    let shared_app = app.clone();
    tauri::async_runtime::spawn_blocking(move || crate::window::paste_into_previous(&shared_app))
        .await
        .map_err(|err| AppError::Internal(err.to_string()))?
}

#[tauri::command]
pub fn clipboard_pin(id: i64, pinned: bool, shared: State<'_, Shared>) -> AppResult<()> {
    store::set_pinned(&shared.db(), id, pinned)
}

#[tauri::command]
pub fn clipboard_delete(id: i64, shared: State<'_, Shared>) -> AppResult<()> {
    let image = store::delete(&shared.db(), id)?;
    if let Some(name) = image {
        images::remove(&shared.dir, &name);
    }
    Ok(())
}

/// Deletes every clip except pinned ones.
#[tauri::command]
pub fn clipboard_clear(shared: State<'_, Shared>) -> AppResult<()> {
    let images = store::clear(&shared.db())?;
    for name in images {
        images::remove(&shared.dir, &name);
    }
    Ok(())
}
