//! Global hotkeys: clipboard history (default Ctrl+Alt+V) and command line
//! (default Alt+Space), both configurable in settings.
//!
//! If the default Alt+Space is taken at startup, Ctrl+Alt+Space is registered
//! instead and the frontend tells the user via a toast.

use std::str::FromStr;
use std::sync::{Mutex, PoisonError};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State, Wry};
use tauri_plugin_global_shortcut::{
    GlobalShortcutExt, Modifiers, Shortcut, ShortcutEvent, ShortcutState,
};

use crate::error::{AppError, AppResult};
use crate::window;

pub const DEFAULT_CLIPBOARD: &str = "Ctrl+Alt+V";
pub const DEFAULT_LAUNCHER: &str = "Alt+Space";
const LAUNCHER_FALLBACK: &str = "Ctrl+Alt+Space";

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

#[derive(Default)]
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

/// Human-readable form used in settings and messages: "Ctrl+Alt+V", "Alt+Space".
pub fn label(shortcut: &Shortcut) -> String {
    let mods = shortcut.mods;
    let mut parts = Vec::new();
    // Windows order: "Win+Shift+S", "Ctrl+Alt+Del".
    for (flag, name) in [
        (Modifiers::SUPER, "Win"),
        (Modifiers::CONTROL, "Ctrl"),
        (Modifiers::ALT, "Alt"),
        (Modifiers::SHIFT, "Shift"),
    ] {
        if mods.contains(flag) {
            parts.push(name.to_owned());
        }
    }
    let key = shortcut.key.to_string();
    let key = key
        .strip_prefix("Key")
        .or_else(|| key.strip_prefix("Digit"))
        .unwrap_or(&key);
    parts.push(key.to_owned());
    parts.join("+")
}

/// Parses "Ctrl+Alt+V"; a hotkey must have a modifier besides Shift.
pub fn parse(text: &str) -> AppResult<Shortcut> {
    let normalized = text.replace("Win+", "Super+");
    let shortcut = Shortcut::from_str(&normalized).map_err(|_| {
        AppError::InvalidArgument(format!("не удалось разобрать сочетание «{text}»"))
    })?;
    let mods = shortcut.mods - Modifiers::SHIFT;
    if mods.is_empty() {
        return Err(AppError::InvalidArgument(format!(
            "«{text}»: нужна клавиша Ctrl, Alt или Win"
        )));
    }
    Ok(shortcut)
}

fn try_register(app: &AppHandle, shortcut: Shortcut) -> bool {
    match app.global_shortcut().register(shortcut) {
        Ok(()) => true,
        Err(err) => {
            log::warn!("failed to register {}: {err}", label(&shortcut));
            false
        }
    }
}

/// Registers the configured hotkeys at startup; failures are reported through
/// `hotkeys_status`, never fatal.
pub fn register(app: &AppHandle, clipboard_key: &str, launcher_key: &str) {
    let clipboard = parse(clipboard_key).ok().filter(|s| try_register(app, *s));
    let mut launcher = parse(launcher_key).ok().filter(|s| try_register(app, *s));
    let mut fallback = false;
    if launcher.is_none() && launcher_key == DEFAULT_LAUNCHER {
        launcher = parse(LAUNCHER_FALLBACK)
            .ok()
            .filter(|s| try_register(app, *s));
        fallback = launcher.is_some();
    }
    let registered = Registered {
        status: HotkeyStatus {
            clipboard: clipboard.as_ref().map(label),
            launcher: launcher.as_ref().map(label),
            launcher_fallback: fallback,
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

/// Switches to new hotkeys. If either cannot be registered (taken by another app),
/// the previous ones are restored and an error explains which key is busy.
pub fn rebind(app: &AppHandle, clipboard_key: &str, launcher_key: &str) -> AppResult<HotkeyStatus> {
    let clipboard = parse(clipboard_key)?;
    let launcher = parse(launcher_key)?;
    if clipboard == launcher {
        return Err(AppError::InvalidArgument(
            "у буфера и командной строки должны быть разные сочетания".into(),
        ));
    }
    let state = app.state::<Mutex<Registered>>();
    let mut registered = state.lock().unwrap_or_else(PoisonError::into_inner);
    let old: Vec<Shortcut> = registered.bindings.iter().map(|(s, _)| *s).collect();
    for shortcut in &old {
        let _ = app.global_shortcut().unregister(*shortcut);
    }

    let mut taken = None;
    let mut new_bindings = Vec::new();
    for (shortcut, action) in [(clipboard, Action::Clipboard), (launcher, Action::Launcher)] {
        if try_register(app, shortcut) {
            new_bindings.push((shortcut, action));
        } else {
            taken = Some(shortcut);
            break;
        }
    }
    if let Some(busy) = taken {
        for (shortcut, _) in &new_bindings {
            let _ = app.global_shortcut().unregister(*shortcut);
        }
        for shortcut in old {
            try_register(app, shortcut);
        }
        return Err(AppError::Unavailable(format!(
            "{} занято другой программой — выберите другое сочетание",
            label(&busy)
        )));
    }

    registered.status = HotkeyStatus {
        clipboard: Some(label(&clipboard)),
        launcher: Some(label(&launcher)),
        launcher_fallback: false,
    };
    registered.bindings = new_bindings;
    Ok(registered.status.clone())
}

/// Temporarily releases (or restores) our hotkeys, so the settings field can record
/// a combination that is currently bound — otherwise the OS would swallow it.
#[tauri::command]
pub fn hotkeys_pause(
    paused: bool,
    app: AppHandle,
    state: State<'_, Mutex<Registered>>,
) -> AppResult<()> {
    let registered = state.lock().unwrap_or_else(PoisonError::into_inner);
    for (shortcut, _) in &registered.bindings {
        if paused {
            let _ = app.global_shortcut().unregister(*shortcut);
        } else if !app.global_shortcut().is_registered(*shortcut) {
            try_register(&app, *shortcut);
        }
    }
    Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_labels_round_trip() {
        for text in [
            "Ctrl+Alt+V",
            "Alt+Space",
            "Ctrl+Shift+K",
            "Win+Alt+1",
            "Ctrl+F5",
        ] {
            assert_eq!(label(&parse(text).unwrap()), text);
        }
        assert_eq!(
            parse("Ctrl+Alt+V").unwrap().key,
            tauri_plugin_global_shortcut::Code::KeyV
        );
    }

    #[test]
    fn rejects_keys_without_a_real_modifier() {
        assert!(parse("V").is_err());
        assert!(parse("Shift+V").is_err());
        assert!(parse("Ctrl+").is_err());
        assert!(parse("Ctrl+Nope").is_err());
    }
}
