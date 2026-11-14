//! Global hotkeys (PLAN.md stage 3–4). Ctrl+Alt+V opens the clipboard history.

use tauri::{AppHandle, Emitter, Wry};
use tauri_plugin_global_shortcut::{
    Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutEvent, ShortcutState,
};

use crate::window;

fn clipboard_shortcut() -> Shortcut {
    Shortcut::new(Some(Modifiers::CONTROL | Modifiers::ALT), Code::KeyV)
}

/// The plugin with our handler; register it before `register`.
pub fn plugin() -> tauri::plugin::TauriPlugin<Wry> {
    tauri_plugin_global_shortcut::Builder::new()
        .with_handler(on_shortcut)
        .build()
}

/// Registers the shortcuts; a failure (key taken by another app) is logged, not fatal.
pub fn register(app: &AppHandle) {
    let shortcut = clipboard_shortcut();
    if let Err(err) = app.global_shortcut().register(shortcut) {
        // Stage 4 adds a toast and a way to pick another key.
        log::warn!("failed to register {shortcut}: {err}");
    }
}

fn on_shortcut(app: &AppHandle, shortcut: &Shortcut, event: ShortcutEvent) {
    if event.state != ShortcutState::Pressed || *shortcut != clipboard_shortcut() {
        return;
    }
    log::debug!("clipboard hotkey pressed");
    if window::is_hidden(app) {
        return;
    }
    if let Err(err) = window::enter_input_mode(app) {
        log::error!("failed to enter input mode: {err}");
    }
    if let Err(err) = app.emit("hotkey://clipboard", ()) {
        log::warn!("failed to emit hotkey://clipboard: {err}");
    }
}
