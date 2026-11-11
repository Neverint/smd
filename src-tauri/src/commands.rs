//! Tauri commands exposed to the frontend. Each one returns `AppResult<T>`.

use serde::Serialize;
use tauri::State;

use crate::error::AppResult;
use crate::window_effect::WindowEffect;

pub struct AppState {
    pub window_effect: WindowEffect,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub version: String,
    pub window_effect: WindowEffect,
}

#[tauri::command]
pub fn app_info(app: tauri::AppHandle, state: State<'_, AppState>) -> AppResult<AppInfo> {
    Ok(AppInfo {
        version: app.package_info().version.to_string(),
        window_effect: state.window_effect,
    })
}

/// Formats a byte count the same way the backend does in logs and notifications.
#[tauri::command]
pub fn format_bytes(bytes: u64) -> AppResult<String> {
    Ok(smd_core::format::format_bytes(bytes))
}
