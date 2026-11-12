//! Settings the backend needs (UI preferences stay in the frontend's storage).

use serde::{Deserialize, Serialize};
use smd_win::cleanup::DeleteMode;
use tauri::State;

use crate::error::{AppError, AppResult};
use crate::state::AppState;
use crate::store;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    /// Files and folders Cleanup never touches.
    pub exclusions: Vec<String>,
    /// Recycle Bin by default. Permanent delete is an explicit opt-in.
    pub delete_mode: DeleteMode,
    /// Removed items go to the disposable drive's quarantine (when it is there and healthy).
    pub quarantine: bool,
    /// Days quarantined items are kept.
    pub quarantine_days: u32,
    /// Purge old quarantine batches automatically: not asked yet (`None`), yes, or no.
    pub purge_consent: Option<bool>,
    /// Files in the scratch folder older than this are listed for cleanup.
    pub scratch_days: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            exclusions: Vec::new(),
            delete_mode: DeleteMode::RecycleBin,
            quarantine: true,
            quarantine_days: 30,
            purge_consent: None,
            scratch_days: 14,
        }
    }
}

pub const FILE: &str = "settings.json";

pub fn load(state: &AppState) -> Settings {
    match store::read_json::<Settings>(&state.data_dir.join(FILE)) {
        Ok(Some(s)) => s,
        Ok(None) => Settings::default(),
        Err(e) => {
            log::warn!("settings file unreadable, using defaults: {e}");
            Settings::default()
        }
    }
}

fn validate(s: &Settings) -> AppResult<()> {
    if !(1..=365).contains(&s.quarantine_days) || !(1..=365).contains(&s.scratch_days) {
        return Err(AppError::Internal("days must be between 1 and 365".into()));
    }
    for path in &s.exclusions {
        let p = std::path::Path::new(path);
        if !p.is_absolute() {
            return Err(AppError::Internal(format!(
                "exclusion is not an absolute path: {path}"
            )));
        }
    }
    Ok(())
}

#[tauri::command]
pub fn settings_get(state: State<'_, AppState>) -> AppResult<Settings> {
    Ok(state
        .settings
        .lock()
        .map_err(|_| AppError::Internal("settings lock".into()))?
        .clone())
}

#[tauri::command]
pub fn settings_set(state: State<'_, AppState>, settings: Settings) -> AppResult<Settings> {
    validate(&settings)?;
    let mut clean = settings;
    clean.exclusions.sort();
    clean.exclusions.dedup();
    store::write_json(&state.data_dir.join(FILE), &clean)?;
    *state
        .settings
        .lock()
        .map_err(|_| AppError::Internal("settings lock".into()))? = clean.clone();
    log::info!(
        "settings saved: {} exclusions, delete mode {:?}",
        clean.exclusions.len(),
        clean.delete_mode
    );
    Ok(clean)
}

/// Opens the local log folder in Explorer.
#[tauri::command]
pub fn open_log_folder(app: tauri::AppHandle) -> AppResult<()> {
    use tauri::Manager;
    let dir = app.path().app_log_dir()?;
    std::fs::create_dir_all(&dir)?;
    tauri_plugin_opener::open_path(dir, None::<&str>).map_err(|e| AppError::Internal(e.to_string()))
}
