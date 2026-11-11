mod commands;
mod error;
mod window_effect;

use tauri::Manager;
use tauri_plugin_log::{RotationStrategy, Target, TargetKind};

/// Keep at most this many rotated log files, each up to 2 MB, in the app's local log folder.
const LOG_FILES_KEPT: usize = 5;
const LOG_FILE_MAX_BYTES: u128 = 2 * 1024 * 1024;

pub fn run() {
    tauri::Builder::default()
        .plugin(
            tauri_plugin_log::Builder::new()
                .targets([
                    Target::new(TargetKind::Stdout),
                    Target::new(TargetKind::LogDir {
                        file_name: Some("smd".into()),
                    }),
                ])
                .max_file_size(LOG_FILE_MAX_BYTES)
                .rotation_strategy(RotationStrategy::KeepSome(LOG_FILES_KEPT))
                .level(log::LevelFilter::Info)
                .build(),
        )
        .setup(|app| {
            let window = app
                .get_webview_window("main")
                .expect("main window is declared in tauri.conf.json");
            let effect = window_effect::apply(&window);
            log::info!("window effect: {effect:?}");
            app.manage(commands::AppState {
                window_effect: effect,
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app_info,
            commands::format_bytes
        ])
        .run(tauri::generate_context!())
        .expect("error while running Scrub My Disks");
}
