mod activity;
mod applications;
mod cleanup;
mod clutter;
mod commands;
mod error;
mod hardware;
mod performance;
mod protection;
mod settings;
mod smart_care;
mod space_lens;
mod state;
mod store;
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
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let window = app
                .get_webview_window("main")
                .expect("main window is declared in tauri.conf.json");
            let effect = window_effect::apply(&window);
            log::info!("window effect: {effect:?}");

            let data_dir = app.path().app_local_data_dir()?;
            std::fs::create_dir_all(&data_dir)?;
            let state = state::AppState::new(effect, data_dir);
            *state.settings.lock().expect("fresh mutex") = settings::load(&state);
            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app_info,
            commands::format_bytes,
            settings::settings_get,
            settings::settings_set,
            settings::open_log_folder,
            activity::activity_list,
            activity::activity_undo,
            cleanup::cleanup_scan,
            cleanup::cleanup_run,
            smart_care::smart_care_scan,
            space_lens::drives_list,
            space_lens::pick_folder,
            space_lens::space_scan,
            space_lens::space_cancel,
            space_lens::space_view,
            space_lens::space_reveal,
            performance::startup_list,
            performance::startup_set,
            performance::processes_snapshot,
            performance::processes_stop,
            performance::background_list,
            performance::open_system_tool,
            applications::apps_list,
            applications::apps_uninstall,
            applications::apps_leftovers,
            applications::apps_remove_leftovers,
            applications::updates_status,
            applications::updates_setup,
            applications::updates_list,
            applications::updates_apply,
            applications::windows_updates,
            clutter::clutter_scan,
            clutter::clutter_cancel,
            clutter::clutter_downloads,
            clutter::clutter_remove,
            protection::protection_status,
            protection::protection_scan,
            protection::protection_cancel_scan,
            protection::protection_update_signatures,
            protection::permission_set,
            hardware::hardware_info,
            hardware::sensors_poll,
            hardware::sensors_stop,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Scrub My Disks");
}
