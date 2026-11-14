//! Application assembly: plugins, commands and startup.

mod clipboard;
pub mod error;
mod hotkeys;
mod launcher;
mod media;
mod settings;
mod storage;
mod system;
mod thread;
mod tray;
mod updater;
mod util;
mod window;

use tauri_plugin_log::{RotationStrategy, Target, TargetKind};

const LOG_FILE_MAX_BYTES: u128 = 5 * 1024 * 1024;

fn log_plugin<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri_plugin_log::Builder::new()
        .targets([
            Target::new(TargetKind::LogDir {
                file_name: Some("escape".into()),
            }),
            Target::new(TargetKind::Stdout),
        ])
        .level(if cfg!(debug_assertions) {
            log::LevelFilter::Debug
        } else {
            log::LevelFilter::Info
        })
        .max_file_size(LOG_FILE_MAX_BYTES)
        .rotation_strategy(RotationStrategy::KeepOne)
        .build()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let result = tauri::Builder::default()
        // single-instance must be registered first so a second launch exits early.
        .plugin(tauri_plugin_single_instance::init(|_app, argv, _cwd| {
            log::info!("second instance launch ignored (argv: {argv:?})");
        }))
        .plugin(log_plugin())
        .plugin(tauri_plugin_opener::init())
        .plugin(hotkeys::plugin())
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(tauri_plugin_updater::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            window::set_island_rect,
            window::set_input_mode,
            system::get_battery,
            media::media_get_state,
            media::media_command,
            clipboard::clipboard_list,
            clipboard::clipboard_copy,
            clipboard::clipboard_paste,
            clipboard::clipboard_pin,
            clipboard::clipboard_delete,
            clipboard::clipboard_clear,
            launcher::launcher_search,
            launcher::launcher_run,
            hotkeys::hotkeys_status,
            settings::settings_get,
            settings::settings_set,
            settings::monitors_list,
            updater::update_check,
            updater::update_install,
        ])
        .setup(|app| {
            log::info!("Escape {} started", env!("CARGO_PKG_VERSION"));
            let handle = app.handle();
            let settings = settings::load(handle);
            window::setup(
                handle,
                settings.monitor.clone(),
                settings.hide_in_fullscreen,
            )?;
            system::spawn_battery_watcher(handle.clone());
            media::start(handle);
            let db = storage::init(handle)?;
            clipboard::start(handle, db.clone(), settings.history_limit as usize)?;
            launcher::start(handle, db)?;
            hotkeys::register(
                handle,
                &settings.clipboard_hotkey,
                &settings.launcher_hotkey,
            );
            tray::create(handle)?;
            Ok(())
        })
        .run(tauri::generate_context!());

    if let Err(err) = result {
        // The logger may not be initialised if startup failed early, so print as well.
        eprintln!("fatal: failed to run Escape: {err}");
        log::error!("fatal: failed to run Escape: {err}");
        std::process::exit(1);
    }
}
