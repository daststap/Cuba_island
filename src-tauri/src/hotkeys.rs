//! Global hotkeys: Ctrl+Alt+V (clipboard history) and Alt+Space (command line).
//!
//! If Alt+Space is taken, Ctrl+Alt+Space is registered instead and the frontend
//! tells the user via a toast (stage 5 adds choosing your own keys).

use std::sync::{Mutex, PoisonError};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State, Wry};
use tauri_plugin_global_shortcut::{
    Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutEvent, ShortcutState,
};

use crate::error::AppResult;
use crate::window;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Action {
    Clipboard,
    Launcher,
}

impl Action {
    fn event(self) -> &'static str {
        match self {
            Self::Clipboard => "hotkey://clipboard",
            Self::Launcher => "hotkey://launcher",
        }
    }
}

/// Which keys ended up registered. Mirrored in `src/ipc/commands.ts`.
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HotkeyStatus {
    /// e.g. "Ctrl+Alt+V"; `None` if it could not be registered.
    pub clipboard: Option<String>,
    pub launcher: Option<String>,
    /// Alt+Space was taken and the fallback is in use.
    pub launcher_fallback: bool,
}

pub struct Registered {
    status: HotkeyStatus,
    bindings: Vec<(Shortcut, Action)>,
}

/// The plugin with our handler; register it before `register`.
pub fn plugin() -> tauri::plugin::TauriPlugin<Wry> {
    tauri_plugin_global_shortcut::Builder::new()
        .with_handler(on_shortcut)
        .build()
}

fn label(shortcut: &Shortcut) -> String {
    let mods = shortcut.mods;
    let mut parts = Vec::new();
    if mods.contains(Modifiers::CONTROL) {
        parts.push("Ctrl");
    }
    if mods.contains(Modifiers::ALT) {
        parts.push("Alt");
    }
    if mods.contains(Modifiers::SHIFT) {
        parts.push("Shift");
    }
    let key = match shortcut.key {
        Code::Space => "Space".to_owned(),
        code => code.to_string().trim_start_matches("Key").to_owned(),
    };
    format!("{}+{key}", parts.join("+"))
}

/// Tries each shortcut in turn; returns the first that registered.
fn register_first(app: &AppHandle, choices: &[Shortcut]) -> Option<Shortcut> {
    choices
        .iter()
        .copied()
        .find(|shortcut| match app.global_shortcut().register(*shortcut) {
            Ok(()) => true,
            Err(err) => {
                log::warn!("failed to register {}: {err}", label(shortcut));
                false
            }
        })
}

/// Registers both hotkeys; failures are reported through `hotkeys_status`, never fatal.
pub fn register(app: &AppHandle) {
    let ctrl_alt = Some(Modifiers::CONTROL | Modifiers::ALT);
    let clipboard = register_first(app, &[Shortcut::new(ctrl_alt, Code::KeyV)]);
    let primary = Shortcut::new(Some(Modifiers::ALT), Code::Space);
    let launcher = register_first(app, &[primary, Shortcut::new(ctrl_alt, Code::Space)]);

    let registered = Registered {
        status: HotkeyStatus {
            clipboard: clipboard.as_ref().map(label),
            launcher: launcher.as_ref().map(label),
            launcher_fallback: launcher.is_some_and(|s| s != primary),
        },
        bindings: clipboard
            .map(|s| (s, Action::Clipboard))
            .into_iter()
            .chain(launcher.map(|s| (s, Action::Launcher)))
            .collect(),
    };
    log::info!("hotkeys: {:?}", registered.status);
    app.manage(Mutex::new(registered));
}

#[tauri::command]
pub fn hotkeys_status(state: State<'_, Mutex<Registered>>) -> AppResult<HotkeyStatus> {
    Ok(state
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .status
        .clone())
}

fn on_shortcut(app: &AppHandle, shortcut: &Shortcut, event: ShortcutEvent) {
    if event.state != ShortcutState::Pressed {
        return;
    }
    let action = app.try_state::<Mutex<Registered>>().and_then(|state| {
        state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .bindings
            .iter()
            .find(|(s, _)| s == shortcut)
            .map(|(_, action)| *action)
    });
    let Some(action) = action else {
        return;
    };
    log::debug!("hotkey {action:?} pressed");
    if window::is_hidden(app) {
        return;
    }
    if let Err(err) = window::enter_input_mode(app) {
        log::error!("failed to enter input mode: {err}");
    }
    if let Err(err) = app.emit(action.event(), ()) {
        log::warn!("failed to emit {}: {err}", action.event());
    }
}
