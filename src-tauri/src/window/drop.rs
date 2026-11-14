//! Our own OLE drop target on the island window (file shelf, PLAN.md stage 6).
//!
//! The island is a transparent composition-hosted WebView2 window without a child
//! HWND, and the drop target Tauri/WebView2 put on it never reported file drags.
//! So we replace it with a minimal `IDropTarget` that accepts files (CF_HDROP).

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

use windows::core::{implement, Ref};
use windows::Win32::Foundation::POINTL;
use windows::Win32::System::Com::{IDataObject, DVASPECT_CONTENT, FORMATETC, TYMED_HGLOBAL};
use windows::Win32::System::Ole::{
    IDropTarget, IDropTarget_Impl, RegisterDragDrop, ReleaseStgMedium, RevokeDragDrop, CF_HDROP,
    DROPEFFECT, DROPEFFECT_COPY, DROPEFFECT_LINK, DROPEFFECT_NONE,
};
use windows::Win32::System::SystemServices::MODIFIERKEYS_FLAGS;
use windows::Win32::UI::Shell::{DragQueryFileW, HDROP};

use super::win32::Hwnd;
use crate::error::{AppError, AppResult};

/// What happened over the island.
pub enum DropEvent {
    /// Files are being dragged over the island.
    Enter,
    Leave,
    Drop(Vec<PathBuf>),
}

type Handler = Box<dyn Fn(DropEvent) + Send + Sync>;

#[implement(IDropTarget)]
struct IslandDropTarget {
    handler: Handler,
    /// The current drag carries files.
    accepted: AtomicBool,
}

fn files_format() -> FORMATETC {
    FORMATETC {
        cfFormat: CF_HDROP.0,
        ptd: std::ptr::null_mut(),
        dwAspect: DVASPECT_CONTENT.0,
        lindex: -1,
        tymed: TYMED_HGLOBAL.0 as u32,
    }
}

fn has_files(data: &IDataObject) -> bool {
    let format = files_format();
    // SAFETY: `format` lives for the call; QueryGetData only reads it.
    unsafe { data.QueryGetData(&format) }.is_ok()
}

fn read_files(data: &IDataObject) -> Vec<PathBuf> {
    let format = files_format();
    // SAFETY: `format` lives for the call; on success we own the medium and release it.
    let Ok(mut medium) = (unsafe { data.GetData(&format) }) else {
        return Vec::new();
    };
    // SAFETY: for TYMED_HGLOBAL the union holds an HGLOBAL with a DROPFILES block.
    let hdrop = HDROP(unsafe { medium.u.hGlobal }.0);
    let mut paths = Vec::new();
    // SAFETY: querying the count (index 0xFFFFFFFF), then each name into a buffer of
    // the reported length + NUL.
    unsafe {
        let count = DragQueryFileW(hdrop, u32::MAX, None);
        for index in 0..count {
            let len = DragQueryFileW(hdrop, index, None) as usize;
            let mut buffer = vec![0_u16; len + 1];
            let copied = DragQueryFileW(hdrop, index, Some(&mut buffer)) as usize;
            paths.push(PathBuf::from(String::from_utf16_lossy(&buffer[..copied])));
        }
        ReleaseStgMedium(&mut medium);
    }
    paths
}

/// Copy if the source allows it (the shelf keeps references, originals stay put).
fn choose_effect(allowed: DROPEFFECT) -> DROPEFFECT {
    if allowed.0 & DROPEFFECT_COPY.0 != 0 {
        DROPEFFECT_COPY
    } else if allowed.0 & DROPEFFECT_LINK.0 != 0 {
        DROPEFFECT_LINK
    } else {
        DROPEFFECT_NONE
    }
}

impl IslandDropTarget {
    fn answer(&self, effect: *mut DROPEFFECT) {
        if effect.is_null() {
            return;
        }
        // SAFETY: OLE passes a valid in/out pointer holding the allowed effects.
        unsafe {
            *effect = if self.accepted.load(Ordering::SeqCst) {
                choose_effect(*effect)
            } else {
                DROPEFFECT_NONE
            };
        }
    }
}

impl IDropTarget_Impl for IslandDropTarget_Impl {
    fn DragEnter(
        &self,
        data: Ref<IDataObject>,
        _keys: MODIFIERKEYS_FLAGS,
        _pt: &POINTL,
        effect: *mut DROPEFFECT,
    ) -> windows::core::Result<()> {
        let files = data.as_ref().is_some_and(has_files);
        self.accepted.store(files, Ordering::SeqCst);
        if files {
            (self.handler)(DropEvent::Enter);
        }
        self.answer(effect);
        Ok(())
    }

    fn DragOver(
        &self,
        _keys: MODIFIERKEYS_FLAGS,
        _pt: &POINTL,
        effect: *mut DROPEFFECT,
    ) -> windows::core::Result<()> {
        self.answer(effect);
        Ok(())
    }

    fn DragLeave(&self) -> windows::core::Result<()> {
        if self.accepted.swap(false, Ordering::SeqCst) {
            (self.handler)(DropEvent::Leave);
        }
        Ok(())
    }

    fn Drop(
        &self,
        data: Ref<IDataObject>,
        _keys: MODIFIERKEYS_FLAGS,
        _pt: &POINTL,
        effect: *mut DROPEFFECT,
    ) -> windows::core::Result<()> {
        self.answer(effect);
        if self.accepted.swap(false, Ordering::SeqCst) {
            let paths = data.as_ref().map(read_files).unwrap_or_default();
            (self.handler)(DropEvent::Drop(paths));
        }
        Ok(())
    }
}

/// Replaces whatever drop target the window has with ours.
/// Must run on the window's thread (OLE is initialised there by tao).
pub fn install(hwnd: Hwnd, handler: impl Fn(DropEvent) + Send + Sync + 'static) -> AppResult<()> {
    let target: IDropTarget = IslandDropTarget {
        handler: Box::new(handler),
        accepted: AtomicBool::new(false),
    }
    .into();
    // SAFETY: our own window on its own thread; revoking a missing target just fails.
    unsafe {
        let _ = RevokeDragDrop(hwnd.raw());
        RegisterDragDrop(hwnd.raw(), &target)
            .map_err(|err| AppError::Internal(format!("RegisterDragDrop: {err}")))?;
    }
    log::info!("island drop target installed");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefers_copy_then_link() {
        let all = DROPEFFECT(DROPEFFECT_COPY.0 | DROPEFFECT_LINK.0 | 2);
        assert_eq!(choose_effect(all), DROPEFFECT_COPY);
        assert_eq!(choose_effect(DROPEFFECT_LINK), DROPEFFECT_LINK);
        assert_eq!(choose_effect(DROPEFFECT(2)), DROPEFFECT_NONE); // move only
    }
}
