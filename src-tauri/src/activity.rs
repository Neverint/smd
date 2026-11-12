//! My Activity: a local history of every scan and action, with undo where possible.
//!
//! `activity.json` keeps the newest entries. For each undoable action, `undo\<id>.json`
//! holds what is needed to put it back: recycled paths, registry backups, a startup item's state.

use serde::{Deserialize, Serialize};
use smd_win::cleanup::{DeleteMode, RestoreReport};
use std::path::{Path, PathBuf};
use tauri::State;

use crate::error::{AppError, AppResult};
use crate::state::{AppState, lock};
use crate::store;

const FILE: &str = "activity.json";
const UNDO_DIR: &str = "undo";
const MAX_ENTRIES: usize = 500;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Kind {
    SmartCareScan,
    CleanupScan,
    Cleanup,
    EmptyRecycleBin,
    SpaceLensScan,
    Restore,
    StartupChange,
    Uninstall,
    LeftoversRemoved,
    AppUpdate,
    UpdatesSetup,
    ClutterRemoved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum Undo {
    #[default]
    None,
    Available,
    Done,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub id: u64,
    pub time_ms: i64,
    pub kind: Kind,
    /// Bytes found (scans), removed (cleanup) or restored.
    #[serde(default)]
    pub bytes: u64,
    #[serde(default)]
    pub items: u64,
    #[serde(default)]
    pub failed: u64,
    #[serde(default)]
    pub score: Option<u8>,
    /// Scanned folder (Space Lens).
    #[serde(default)]
    pub target: Option<String>,
    #[serde(default)]
    pub mode: Option<DeleteMode>,
    /// Startup item turned on (true) or off (false).
    #[serde(default)]
    pub enabled: Option<bool>,
    #[serde(default)]
    pub undo: Undo,
}

impl Entry {
    pub fn new(id: u64, kind: Kind) -> Self {
        Entry {
            id,
            time_ms: store::now_ms(),
            kind,
            bytes: 0,
            items: 0,
            failed: 0,
            score: None,
            target: None,
            mode: None,
            enabled: None,
            undo: Undo::None,
        }
    }
}

/// A registry key exported before it was deleted.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegBackup {
    pub key: String,
    pub file: PathBuf,
}

/// A startup item's state before it was changed.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartupUndo {
    pub id: String,
    pub was_enabled: bool,
}

/// What an undo needs. Phase 1 files (paths only) still load.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UndoData {
    #[serde(default)]
    pub started_unix_s: i64,
    /// Moved to the Recycle Bin.
    #[serde(default)]
    pub paths: Vec<String>,
    #[serde(default)]
    pub registry: Vec<RegBackup>,
    #[serde(default)]
    pub startup: Option<StartupUndo>,
}

fn file(dir: &Path) -> PathBuf {
    dir.join(FILE)
}

fn load(dir: &Path) -> Vec<Entry> {
    store::read_json::<Vec<Entry>>(&file(dir))
        .ok()
        .flatten()
        .unwrap_or_default()
}

/// Appends an entry (newest first), keeping at most `MAX_ENTRIES`.
pub fn record(state: &AppState, entry: Entry) {
    let Ok(_guard) = state.activity_lock.lock() else {
        return;
    };
    let mut all = load(&state.data_dir);
    all.insert(0, entry);
    for dropped in all.drain(MAX_ENTRIES.min(all.len())..) {
        let _ = std::fs::remove_file(undo_path(&state.data_dir, dropped.id));
        let _ = std::fs::remove_dir_all(undo_files_dir(state, dropped.id));
    }
    if let Err(e) = store::write_json(&file(&state.data_dir), &all) {
        log::warn!("could not save activity: {e}");
    }
}

fn undo_path(dir: &Path, id: u64) -> PathBuf {
    dir.join(UNDO_DIR).join(format!("{id}.json"))
}

/// Saves the recycled paths so the action can be undone later.
pub fn save_undo(state: &AppState, id: u64, data: &UndoData) -> bool {
    store::write_json(&undo_path(&state.data_dir, id), data).is_ok()
}

/// Folder for files an undo needs (registry backups).
pub fn undo_files_dir(state: &AppState, id: u64) -> PathBuf {
    state.data_dir.join(UNDO_DIR).join(id.to_string())
}

#[tauri::command]
pub fn activity_list(state: State<'_, AppState>) -> AppResult<Vec<Entry>> {
    let _guard = lock(&state.activity_lock)?;
    Ok(load(&state.data_dir))
}

/// Puts the items of a cleanup back from the Recycle Bin.
#[tauri::command]
pub async fn activity_undo(app: tauri::AppHandle, id: u64) -> AppResult<RestoreReport> {
    tauri::async_runtime::spawn_blocking(move || {
        use tauri::Manager;
        let state = app.state::<AppState>();
        let _busy = state.begin("undo")?;
        let entry = {
            let _guard = lock(&state.activity_lock)?;
            load(&state.data_dir).into_iter().find(|e| e.id == id)
        }
        .ok_or_else(|| AppError::NotFound(format!("activity {id}")))?;
        if entry.undo != Undo::Available {
            return Err(AppError::Stale("this action cannot be undone".into()));
        }
        let undo: UndoData = store::read_json(&undo_path(&state.data_dir, id))?
            .ok_or_else(|| AppError::NotFound(format!("undo data for {id}")))?;
        let mut report = if undo.paths.is_empty() {
            RestoreReport::default()
        } else {
            smd_win::cleanup::restore(&undo.paths, undo.started_unix_s)?
        };
        for b in &undo.registry {
            match smd_win::apps::restore_key(&b.file) {
                Ok(()) => report.restored += 1,
                Err(e) => {
                    log::warn!("could not restore {}: {e}", b.key);
                    report.failed += 1;
                }
            }
        }
        if let Some(s) = &undo.startup {
            match smd_win::startup::set_enabled(&s.id, s.was_enabled, store::now_ms()) {
                Ok(_) => report.restored += 1,
                Err(e) => {
                    log::warn!("could not undo startup change {}: {e}", s.id);
                    report.failed += 1;
                }
            }
        }
        log::info!(
            "undo {id}: restored {}, missing {}, failed {}",
            report.restored,
            report.missing,
            report.failed
        );

        {
            let _guard = lock(&state.activity_lock)?;
            let mut all = load(&state.data_dir);
            if let Some(e) = all.iter_mut().find(|e| e.id == id) {
                e.undo = Undo::Done;
            }
            store::write_json(&file(&state.data_dir), &all)?;
        }
        let mut restore = Entry::new(state.next_id(), Kind::Restore);
        restore.items = report.restored;
        restore.failed = report.failed + report.missing;
        record(&state, restore);
        Ok(report)
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entry_json_is_camel_case_and_tolerant() {
        let e = Entry::new(7, Kind::EmptyRecycleBin);
        let json = serde_json::to_value(&e).unwrap();
        assert_eq!(json["kind"], "emptyRecycleBin");
        assert_eq!(json["undo"], "none");
        // Older files without the optional fields still load.
        let old: Entry = serde_json::from_str(r#"{"id":1,"timeMs":5,"kind":"cleanup"}"#).unwrap();
        assert_eq!(old.undo, Undo::None);
    }
}
