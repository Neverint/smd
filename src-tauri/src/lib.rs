mod activity;
mod applications;
mod cleanup;
mod clutter;
mod commands;
mod error;
mod guard;
mod hardware;
mod helper;
mod notify;
mod organizer;
mod performance;
mod protection;
mod removal;
mod schedule;
mod settings;
mod smart_care;
mod space_lens;
mod state;
mod store;
mod toolbox;
mod tray;
mod tweaks;
mod window_effect;

use tauri::{Manager, WindowEvent};

pub use helper::helper_main;
use tauri_plugin_log::{RotationStrategy, Target, TargetKind};

/// Keep at most this many rotated log files, each up to 2 MB, in the app's local log folder.
const LOG_FILES_KEPT: usize = 5;
const LOG_FILE_MAX_BYTES: u128 = 2 * 1024 * 1024;

pub fn run() {
    // Started by the scheduled task: scan in the background, notify, quit.
    let silent = std::env::args().any(|a| a == "--smart-care");
    tauri::Builder::default()
        // First: a second start (scheduled task, double-click) goes to the running app.
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            if args.iter().any(|a| a == "--smart-care") {
                tray::run_smart_care(app, false);
            } else {
                tray::show_main(app);
            }
        }))
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
        .plugin(tauri_plugin_notification::init())
        .setup(move |app| {
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
            // Purge old quarantine batches in the background (only after the user said yes once).
            let handle = app.handle().clone();
            std::thread::spawn(move || {
                organizer::purge_old(&handle.state::<state::AppState>());
            });
            tray::setup(app.handle())?;
            if silent {
                tray::run_smart_care(app.handle(), true);
            } else {
                window.show()?;
                guard::start(app.handle());
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() == "main"
                && let WindowEvent::CloseRequested { api, .. } = event
            {
                tray::on_main_close(window.app_handle(), api);
            }
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
            organizer::organizer_audit,
            organizer::organizer_policy_set,
            organizer::organizer_plan,
            organizer::organizer_run,
            organizer::organizer_resume,
            organizer::organizer_cancel,
            organizer::quarantine_list,
            organizer::quarantine_restore,
            organizer::quarantine_purge_consent,
            clutter::clutter_scratch,
            cleanup::cleanup_admin_scan,
            hardware::disk_reliability,
            hardware::sensors_admin_poll,
            hardware::sensors_admin_stop,
            tweaks::tweaks_list,
            tweaks::tweaks_apply,
            tweaks::tweak_undo,
            tweaks::restart_explorer,
            tweaks::admin_session_end,
            toolbox::toolbox_list,
            toolbox::toolbox_install,
            toolbox::toolbox_open,
            tray::tray_info,
            tray::tray_sample,
            tray::tray_action,
            tray::tray_hide,
            schedule::schedule_get,
            schedule::schedule_set,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Scrub My Disks");
}
