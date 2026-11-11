//! My Activity: a local history of every scan and action, with undo where possible.
//!
//! `activity.json` keeps the newest entries. For each undoable cleanup,
//! `undo\<id>.json` lists the paths moved to the Recycle Bin.

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
            undo: Undo::None,
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UndoFile {
    started_unix_s: i64,
    paths: Vec<String>,
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
    }
    if let Err(e) = store::write_json(&file(&state.data_dir), &all) {
        log::warn!("could not save activity: {e}");
    }
}

fn undo_path(dir: &Path, id: u64) -> PathBuf {
    dir.join(UNDO_DIR).join(format!("{id}.json"))
}

/// Saves the recycled paths so the action can be undone later.
pub fn save_undo(state: &AppState, id: u64, started_unix_s: i64, paths: Vec<String>) -> bool {
    store::write_json(
        &undo_path(&state.data_dir, id),
        &UndoFile {
            started_unix_s,
            paths,
        },
    )
    .is_ok()
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
        let undo: UndoFile = store::read_json(&undo_path(&state.data_dir, id))?
            .ok_or_else(|| AppError::NotFound(format!("undo data for {id}")))?;
        let report = smd_win::cleanup::restore(&undo.paths, undo.started_unix_s)?;
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
