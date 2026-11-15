//! Listens for clipboard changes and records them (PLAN.md stage 3).

use clipboard_master::{CallbackResult, ClipboardHandler, Master};

use super::store::{self, ClipKind, NewClip, MAX_TEXT_BYTES};
use super::{win32, ClipboardShared};
use crate::error::AppResult;

const SETTLE_DELAY: std::time::Duration = std::time::Duration::from_millis(75);

struct Handler<F: Fn(store::ClipItem)> {
    shared: std::sync::Arc<ClipboardShared>,
    on_added: F,
}

impl<F: Fn(store::ClipItem)> ClipboardHandler for Handler<F> {
    fn on_clipboard_change(&mut self) -> CallbackResult {
        // Apps often write several formats in a row; reading at once makes their
        // later clipboard calls fail ("clipboard busy"). Let them finish first.
        std::thread::sleep(SETTLE_DELAY);
        if win32::written_by_us() {
            return CallbackResult::Next;
        }
        if win32::is_private() {
            log::debug!("clipboard content marked private, skipped");
            return CallbackResult::Next;
        }
        match record(&self.shared) {
            Ok(Some(item)) => (self.on_added)(item),
            Ok(None) => {}
            Err(err) => log::warn!("failed to record clipboard change: {err}"),
        }
        CallbackResult::Next
    }

    fn on_clipboard_error(&mut self, error: std::io::Error) -> CallbackResult {
        log::warn!("clipboard listener error: {error}");
        CallbackResult::Next
    }
}

/// Runs the clipboard listener on the current thread until it fails.
pub fn run(
    shared: std::sync::Arc<ClipboardShared>,
    on_added: impl Fn(store::ClipItem),
) -> std::io::Result<()> {
    Master::new(Handler { shared, on_added })?.run()
}

/// Reads the clipboard and stores text (preferred) or an image. `None` if nothing usable.
fn record(shared: &ClipboardShared) -> AppResult<Option<store::ClipItem>> {
    let mut clipboard = arboard::Clipboard::new()
        .map_err(|err| crate::error::AppError::Internal(err.to_string()))?;

    if let Ok(text) = clipboard.get_text() {
        if text.trim().is_empty() || text.len() > MAX_TEXT_BYTES {
            return Ok(None);
        }
        let hash = store::content_hash(ClipKind::Text, text.as_bytes());
        return shared.insert(NewClip::Text(&text), &hash).map(Some);
    }

    if let Ok(image) = clipboard.get_image() {
        let (width, height) = (
            u32::try_from(image.width).unwrap_or(0),
            u32::try_from(image.height).unwrap_or(0),
        );
        if width == 0 || height == 0 {
            return Ok(None);
        }
        return shared
            .insert_image(width, height, image.bytes.into_owned())
            .map(Some);
    }

    // Files from Explorer and other formats are not recorded in v1.
    Ok(None)
}
