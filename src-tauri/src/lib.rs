//! Application assembly: plugins, commands and startup.

pub mod error;
mod system;
mod thread;
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
        .invoke_handler(tauri::generate_handler![
            window::set_island_rect,
            system::get_battery,
        ])
        .setup(|app| {
            log::info!("Escape {} started", env!("CARGO_PKG_VERSION"));
            window::setup(app.handle())?;
            system::spawn_battery_watcher(app.handle().clone());
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
