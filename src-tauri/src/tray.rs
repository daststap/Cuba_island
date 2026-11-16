//! Tray icon with Настройки / Перезапустить / Выход (PLAN.md stage 5).
//!
//! The menu is native, so its labels live here rather than in `ru.ts`.

use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Emitter};

use crate::error::{AppError, AppResult};

pub fn create(app: &AppHandle) -> AppResult<()> {
    let settings = MenuItem::with_id(app, "settings", "Настройки", true, None::<&str>)?;
    let restart = MenuItem::with_id(app, "restart", "Перезапустить", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Выход", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let menu = Menu::with_items(app, &[&settings, &separator, &restart, &quit])?;
    let icon = app
        .default_window_icon()
        .cloned()
        .ok_or_else(|| AppError::Internal("нет иконки приложения".into()))?;

    TrayIconBuilder::with_id("cuba")
        .icon(icon)
        .tooltip("Cuba")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "settings" => {
                if let Err(err) = app.emit("tray://settings", ()) {
                    log::warn!("failed to emit tray://settings: {err}");
                }
            }
            "restart" => app.restart(),
            "quit" => app.exit(0),
            _ => {}
        })
        .build(app)?;
    Ok(())
}
