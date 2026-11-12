//! My Clutter: scan your folders (read-only), then move what you pick to the Recycle Bin.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use serde::Serialize;
use smd_core::safety;
use smd_win::cleanup::{DeleteMode, Failure, Item, remove_items};
use smd_win::clutter::{ClutterOptions, ClutterProgress, ClutterResult, FolderItem};
use smd_win::shell::KnownFolder;
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager};

use crate::activity::{self, Entry, Undo, UndoData};
use crate::error::{AppError, AppResult};
use crate::state::{AppState, ClutterScan, DownloadsList, lock};
use crate::store::now_ms;

async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> AppResult<T> + Send + 'static,
) -> AppResult<T> {
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
}

/// Your own folders: Desktop, Documents, Downloads, Pictures, Music, Videos (redirected ones included).
pub fn user_folders() -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = [
        KnownFolder::Desktop,
        KnownFolder::Documents,
        KnownFolder::Downloads,
        KnownFolder::Pictures,
        KnownFolder::Music,
        KnownFolder::Videos,
    ]
    .into_iter()
    .filter_map(smd_win::shell::known_folder)
    .filter(|p| p.is_dir())
    .collect();
    // Drop folders inside another one in the list (Documents inside Desktop...).
    let all = v.clone();
    v.retain(|p| {
        !all.iter()
            .any(|o| o != p && safety::is_inside(&p.to_string_lossy(), &o.to_string_lossy()))
    });
    v.dedup();
    v
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClutterScanProgress {
    /// `walk` or `compare`
    pub stage: &'static str,
    pub files: u64,
    pub bytes: u64,
    pub hashed: u64,
    pub to_hash: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClutterView {
    pub scan_id: u64,
    pub roots: Vec<String>,
    pub result: ClutterResult,
    pub duration_ms: u64,
}

/// Scans `path`, or your own folders when `path` is empty. Read-only.
#[tauri::command]
pub async fn clutter_scan(
    app: AppHandle,
    path: Option<String>,
    on_progress: Channel<ClutterScanProgress>,
) -> AppResult<ClutterView> {
    blocking(move || {
        let state = app.state::<AppState>();
        let _busy = state.begin_clutter()?;
        let roots = match path.filter(|p| !p.trim().is_empty()) {
            Some(p) => {
                let pb = PathBuf::from(&p);
                if !pb.is_dir() {
                    return Err(AppError::NotFound(p));
                }
                vec![pb]
            }
            None => user_folders(),
        };
        state.clutter_cancel.store(false, Ordering::Relaxed);
        let started = now_ms();
        let progress = ClutterProgress::default();
        let finished = AtomicBool::new(false);
        let send = || {
            let _ = on_progress.send(ClutterScanProgress {
                stage: if progress.stage.load(Ordering::Relaxed) == 0 {
                    "walk"
                } else {
                    "compare"
                },
                files: progress.files.load(Ordering::Relaxed),
                bytes: progress.bytes.load(Ordering::Relaxed),
                hashed: progress.hashed.load(Ordering::Relaxed),
                to_hash: progress.to_hash.load(Ordering::Relaxed),
            });
        };
        let result = std::thread::scope(|s| {
            s.spawn(|| {
                while !finished.load(Ordering::Relaxed) {
                    std::thread::sleep(Duration::from_millis(200));
                    send();
                }
            });
            let r = smd_win::clutter::scan(
                &roots,
                ClutterOptions::default(),
                started,
                &progress,
                &state.clutter_cancel,
            );
            finished.store(true, Ordering::Relaxed);
            r
        });
        send();
        let scan_id = state.next_id();
        if !result.cancelled {
            *lock(&state.clutter)? = Some(ClutterScan {
                id: scan_id,
                result: result.clone(),
            });
        }
        log::info!(
            "clutter scan {scan_id}: {} files, {} large, {} old, {} duplicate groups",
            result.files_scanned,
            result.large.len(),
            result.old.len(),
            result.duplicates.len()
        );
        Ok(ClutterView {
            scan_id,
            roots: roots
                .iter()
                .map(|r| r.to_string_lossy().into_owned())
                .collect(),
            result,
            duration_ms: (now_ms() - started).max(0) as u64,
        })
    })
    .await
}

#[tauri::command]
pub fn clutter_cancel(state: tauri::State<'_, AppState>) -> AppResult<()> {
    state.clutter_cancel.store(true, Ordering::Relaxed);
    Ok(())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadsView {
    pub scan_id: u64,
    pub path: String,
    pub items: Vec<FolderItem>,
}

/// Everything in your Downloads folder, with size and age. Read-only.
#[tauri::command]
pub async fn clutter_downloads(app: AppHandle) -> AppResult<DownloadsView> {
    blocking(move || {
        let state = app.state::<AppState>();
        let root = smd_win::shell::known_folder(KnownFolder::Downloads)
            .ok_or_else(|| AppError::NotFound("Downloads folder".into()))?;
        let items = smd_win::clutter::folder_items(&root)?;
        let scan_id = state.next_id();
        *lock(&state.downloads)? = Some(DownloadsList {
            id: scan_id,
            root: root.clone(),
            items: items.clone(),
        });
        Ok(DownloadsView {
            scan_id,
            path: root.to_string_lossy().into_owned(),
            items,
        })
    })
    .await
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClutterReport {
    pub removed_bytes: u64,
    pub removed_items: u64,
    pub failures: Vec<Failure>,
    pub undo_available: bool,
}

/// Checks a removal request against a scan: every path must be in it, and no duplicate group may lose all its copies.
pub fn plan_removal(result: &ClutterResult, paths: &[String]) -> AppResult<Vec<Item>> {
    let wanted: HashSet<String> = paths.iter().map(|p| safety::normalize(p)).collect();
    let mut known: HashMap<String, Item> = HashMap::new();
    let lists = result
        .large
        .iter()
        .chain(result.old.iter())
        .chain(result.duplicates.iter().flat_map(|g| g.files.iter()));
    for f in lists {
        known
            .entry(safety::normalize(&f.path))
            .or_insert_with(|| Item {
                path: f.path.clone(),
                name: f.name.clone(),
                bytes: f.bytes,
                files: 1,
                modified_ms: f.modified_ms,
                is_dir: false,
                root: f.root.clone(),
                item_is_root: false,
            });
    }
    for g in &result.duplicates {
        if g.files
            .iter()
            .all(|f| wanted.contains(&safety::normalize(&f.path)))
        {
            return Err(AppError::AccessDenied(format!(
                "at least one copy of {} must stay",
                g.files[0].name
            )));
        }
    }
    wanted
        .iter()
        .map(|p| {
            known.get(p).cloned().ok_or_else(|| {
                AppError::Stale(format!("{p} is not in the current results; scan again"))
            })
        })
        .collect()
}

fn finish(state: &AppState, items: &[Item], kind_target: &str) -> ClutterReport {
    let started = now_ms();
    let outcome = remove_items(items, DeleteMode::RecycleBin);
    let id = state.next_id();
    let removed_bytes = outcome.removed.iter().map(|r| r.1).sum();
    let undo = UndoData {
        started_unix_s: started / 1000,
        paths: outcome.removed.iter().map(|r| r.0.clone()).collect(),
        ..Default::default()
    };
    let undo_available = !outcome.removed.is_empty() && activity::save_undo(state, id, &undo);
    let mut e = Entry::new(id, activity::Kind::ClutterRemoved);
    e.target = Some(kind_target.to_string());
    e.bytes = removed_bytes;
    e.items = outcome.removed.len() as u64;
    e.failed = outcome.failed.len() as u64;
    e.undo = if undo_available {
        Undo::Available
    } else {
        Undo::None
    };
    activity::record(state, e);
    ClutterReport {
        removed_bytes,
        removed_items: outcome.removed.len() as u64,
        failures: outcome.failed,
        undo_available,
    }
}

/// Moves the files you picked to the Recycle Bin (always; permanent delete is not offered here).
#[tauri::command]
pub async fn clutter_remove(
    app: AppHandle,
    scan_id: u64,
    paths: Vec<String>,
) -> AppResult<ClutterReport> {
    blocking(move || {
        let state = app.state::<AppState>();
        let _busy = state.begin("a My Clutter cleanup")?;
        // A scan result...
        let from_scan = {
            let slot = lock(&state.clutter)?;
            match slot.as_ref() {
                Some(s) if s.id == scan_id => Some(plan_removal(&s.result, &paths)?),
                _ => None,
            }
        };
        if let Some(items) = from_scan {
            let report = finish(&state, &items, "scan");
            *lock(&state.clutter)? = None;
            return Ok(report);
        }
        // ...or the Downloads list.
        let items = {
            let slot = lock(&state.downloads)?;
            let list = slot
                .as_ref()
                .filter(|d| d.id == scan_id)
                .ok_or_else(|| AppError::Stale("these results were replaced; scan again".into()))?;
            let wanted: HashSet<String> = paths.iter().map(|p| safety::normalize(p)).collect();
            let root = list.root.to_string_lossy().into_owned();
            list.items
                .iter()
                .filter(|i| wanted.contains(&safety::normalize(&i.path)))
                .map(|i| Item {
                    path: i.path.clone(),
                    name: i.name.clone(),
                    bytes: i.bytes,
                    files: i.files,
                    modified_ms: i.modified_ms,
                    is_dir: i.is_dir,
                    root: root.clone(),
                    item_is_root: false,
                })
                .collect::<Vec<_>>()
        };
        let report = finish(&state, &items, "downloads");
        *lock(&state.downloads)? = None;
        Ok(report)
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use smd_win::clutter::{DuplicateGroup, FileEntry};

    fn f(path: &str) -> FileEntry {
        FileEntry {
            path: path.into(),
            name: path.rsplit('\\').next().unwrap().into(),
            bytes: 10,
            modified_ms: None,
            root: r"D:\data".into(),
        }
    }

    #[test]
    fn keeps_one_copy_and_refuses_unknown_paths() {
        let result = ClutterResult {
            large: vec![f(r"D:\data\big.iso")],
            old: vec![],
            duplicates: vec![DuplicateGroup {
                bytes: 10,
                files: vec![f(r"D:\data\a.jpg"), f(r"D:\data\copy\a.jpg")],
            }],
            duplicate_bytes: 10,
            files_scanned: 3,
            bytes_scanned: 30,
            cancelled: false,
        };
        let one = plan_removal(
            &result,
            &[r"D:\data\copy\a.jpg".into(), r"d:\DATA\big.iso".into()],
        )
        .unwrap();
        assert_eq!(one.len(), 2);
        assert!(one.iter().all(|i| i.root == r"D:\data"));
        assert!(matches!(
            plan_removal(
                &result,
                &[r"D:\data\a.jpg".into(), r"D:\data\copy\a.jpg".into()]
            ),
            Err(AppError::AccessDenied(_))
        ));
        assert!(matches!(
            plan_removal(&result, &[r"C:\Windows\explorer.exe".into()]),
            Err(AppError::Stale(_))
        ));
    }
}
