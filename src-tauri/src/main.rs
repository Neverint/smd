// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // `smd.exe --helper <pipe> <pid>`: the elevated helper, never the app.
    if let Some(code) = smd_lib::helper_main() {
        std::process::exit(code);
    }
    smd_lib::run()
}
