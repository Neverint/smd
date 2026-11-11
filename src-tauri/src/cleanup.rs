//! Cleanup commands: scan (read-only) and run (only what the user kept selected).

use std::collections::HashSet;
use std::sync::atomic::{AtomicU32, Ordering};

use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use smd_core::cleanup::builtin_rules;
use smd_win::cleanup::{Category, DeleteMode, Failure, ScanEnv, remove_items, scan_rule};
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager};

use crate::activity::{self, Entry, Undo};
use crate::error::{AppError, AppResult};
use crate::state::{AppState, CleanupScan, lock};
use crate::store::now_ms;

/// The review list shows the largest items; the rest are summed in one line.
const MAX_ITEMS_SENT: usize = 1000;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanProgress {
    /// Rule id or check name that just finished.
    pub step: String,
    pub done: u32,
    pub total: u32,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoryView {
    #[serde(flatten)]
    pub category: Category,
    /// Items not listed (smaller than the listed ones). They are removed with the category.
    pub more_items: u64,
    pub more_bytes: u64,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanupScanResult {
    pub scan_id: u64,
    pub categories: Vec<CategoryView>,
    /// Space that can be freed now (selectable categories, without the Recycle Bin).
    pub reclaimable_bytes: u64,
    pub duration_ms: u64,
}

/// Lower-case environment lookup for rule variables.
fn env_var(name: &str) -> Option<String> {
    std::env::var(name).ok()
}

/// Scans every rule in parallel and stores the result as the current scan.
pub fn scan_all(state: &AppState, mut progress: impl FnMut(&str)) -> CleanupScanResult {
    let started = now_ms();
    let rules = builtin_rules();
    let exclusions = lock(&state.settings)
        .map(|s| s.exclusions.clone())
        .unwrap_or_default();
    let running: HashSet<String> = smd_win::system::running_process_names();
    let var = env_var;
    let done = AtomicU32::new(0);
    let (tx, rx) = std::sync::mpsc::channel::<String>();

    let categories: Vec<Category> = std::thread::scope(|s| {
        let worker = s.spawn(|| {
            let env = ScanEnv {
                var: &var,
                exclusions: &exclusions,
                running: &running,
                now_ms: started,
            };
            rules
                .par_iter()
                .map_with(tx, |tx, rule| {
                    let c = scan_rule(rule, &env);
                    let _ = tx.send(rule.id.clone());
                    c
                })
                .collect::<Vec<_>>()
        });
        for id in rx {
            done.fetch_add(1, Ordering::Relaxed);
            progress(&id);
        }
        worker.join().unwrap_or_default()
    });

    let reclaimable = categories
        .iter()
        .filter(|c| !c.admin && !c.permanent)
        .map(|c| c.bytes)
        .sum();
    let id = state.next_id();
    let views = categories.iter().cloned().map(to_view).collect();
    if let Ok(mut slot) = lock(&state.cleanup) {
        *slot = Some(CleanupScan { id, categories });
    }
    CleanupScanResult {
        scan_id: id,
        categories: views,
        reclaimable_bytes: reclaimable,
        duration_ms: (now_ms() - started).max(0) as u64,
    }
}

fn to_view(mut category: Category) -> CategoryView {
    let (mut more_items, mut more_bytes) = (0, 0);
    if category.items.len() > MAX_ITEMS_SENT {
        for item in category.items.split_off(MAX_ITEMS_SENT) {
            more_items += 1;
            more_bytes += item.bytes;
        }
    }
    CategoryView {
        category,
        more_items,
        more_bytes,
    }
}

pub fn rule_count() -> u32 {
    builtin_rules().len() as u32
}

#[tauri::command]
pub async fn cleanup_scan(
    app: AppHandle,
    on_progress: Channel<ScanProgress>,
) -> AppResult<CleanupScanResult> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let _busy = state.begin("a cleanup scan")?;
        let total = rule_count();
        let mut done = 0;
        let result = scan_all(&state, |step| {
            done += 1;
            let _ = on_progress.send(ScanProgress {
                step: step.to_string(),
                done,
                total,
            });
        });
        log::info!(
            "cleanup scan {}: {} reclaimable in {} ms",
            result.scan_id,
            result.reclaimable_bytes,
            result.duration_ms
        );
        let mut e = Entry::new(state.next_id(), activity::Kind::CleanupScan);
        e.bytes = result.reclaimable_bytes;
        e.items = result
            .categories
            .iter()
            .map(|c| c.category.items.len() as u64 + c.more_items)
            .sum();
        activity::record(&state, e);
        Ok(result)
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))?
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Selection {
    pub id: String,
    /// Paths the user unticked in the review list.
    #[serde(default)]
    pub skip: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanupReport {
    pub mode: DeleteMode,
    pub removed_bytes: u64,
    pub removed_items: u64,
    pub failed_count: u64,
    /// First failures, for the details list.
    pub failures: Vec<Failure>,
    /// Size of the Recycle Bin when it was emptied (`None` if it was not selected).
    pub bin_emptied_bytes: Option<u64>,
    pub undo_available: bool,
    pub activity_id: u64,
}

/// Runs a cleanup on the current scan. Admin categories are refused (elevated helper: Phase 4).
pub fn run(state: &AppState, scan_id: u64, selections: &[Selection]) -> AppResult<CleanupReport> {
    let scan = lock(&state.cleanup)?
        .take()
        .ok_or_else(|| AppError::Stale("no scan results; scan again".into()))?;
    if scan.id != scan_id {
        // Keep the newer scan for the UI that owns it.
        let newer = scan;
        *lock(&state.cleanup)? = Some(newer);
        return Err(AppError::Stale(format!(
            "scan {scan_id} was replaced; scan again"
        )));
    }
    let mode = lock(&state.settings)?.delete_mode;
    let started_ms = now_ms();

    let mut items = Vec::new();
    let mut empty_bin = false;
    for sel in selections {
        let Some(cat) = scan.categories.iter().find(|c| c.id == sel.id) else {
            continue;
        };
        if cat.admin {
            continue;
        }
        if cat.permanent {
            empty_bin = true;
            continue;
        }
        let skip: HashSet<&str> = sel.skip.iter().map(String::as_str).collect();
        items.extend(
            cat.items
                .iter()
                .filter(|i| !skip.contains(i.path.as_str()))
                .cloned(),
        );
    }

    // Empty the bin first, so what we move there now can still be restored.
    let mut bin_emptied_bytes = None;
    if empty_bin {
        let (bytes, _) = smd_win::system::recycle_bin_size().unwrap_or((0, 0));
        smd_win::system::empty_recycle_bin()?;
        bin_emptied_bytes = Some(bytes);
        let mut e = Entry::new(state.next_id(), activity::Kind::EmptyRecycleBin);
        e.bytes = bytes;
        activity::record(state, e);
        log::info!("emptied the Recycle Bin ({bytes} bytes)");
    }

    let outcome = remove_items(&items, mode);
    let removed_bytes = outcome.removed.iter().map(|r| r.1).sum();
    let id = state.next_id();
    let undo_available = mode == DeleteMode::RecycleBin
        && !outcome.removed.is_empty()
        && activity::save_undo(
            state,
            id,
            started_ms / 1000,
            outcome.removed.iter().map(|r| r.0.clone()).collect(),
        );

    let mut e = Entry::new(id, activity::Kind::Cleanup);
    e.bytes = removed_bytes;
    e.items = outcome.removed.len() as u64;
    e.failed = outcome.failed.len() as u64;
    e.mode = Some(mode);
    e.undo = if undo_available {
        Undo::Available
    } else {
        Undo::None
    };
    if !items.is_empty() {
        activity::record(state, e);
    }
    log::info!(
        "cleanup {id}: removed {} items ({removed_bytes} bytes), {} skipped, mode {mode:?}",
        outcome.removed.len(),
        outcome.failed.len()
    );

    Ok(CleanupReport {
        mode,
        removed_bytes,
        removed_items: outcome.removed.len() as u64,
        failed_count: outcome.failed.len() as u64,
        failures: outcome.failed.into_iter().take(100).collect(),
        bin_emptied_bytes,
        undo_available,
        activity_id: id,
    })
}

#[tauri::command]
pub async fn cleanup_run(
    app: AppHandle,
    scan_id: u64,
    selections: Vec<Selection>,
) -> AppResult<CleanupReport> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let _busy = state.begin("a cleanup")?;
        run(&state, scan_id, &selections)
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))?
}

#[cfg(test)]
mod tests {
    use super::*;
    use smd_win::cleanup::Item;
    use std::io::Write;

    fn state(dir: &std::path::Path) -> AppState {
        AppState::new(crate::window_effect::WindowEffect::None, dir.to_path_buf())
    }

    fn category(id: &str, items: Vec<Item>) -> Category {
        Category {
            id: id.into(),
            group: smd_core::cleanup::Group::System,
            safety: smd_core::cleanup::Safety::Safe,
            admin: false,
            permanent: false,
            default_selected: true,
            bytes: items.iter().map(|i| i.bytes).sum(),
            files: items.len() as u64,
            items,
            excluded: 0,
            running: vec![],
            unreadable: false,
        }
    }

    fn item(root: &std::path::Path, name: &str, bytes: usize) -> Item {
        let path = root.join(name);
        std::fs::File::create(&path)
            .unwrap()
            .write_all(&vec![0u8; bytes])
            .unwrap();
        Item {
            path: path.to_string_lossy().into_owned(),
            name: name.into(),
            bytes: bytes as u64,
            files: 1,
            modified_ms: None,
            is_dir: false,
            root: root.to_string_lossy().into_owned(),
            item_is_root: false,
        }
    }

    #[test]
    fn runs_only_selected_items_of_the_current_scan() {
        let data = tempfile::tempdir().unwrap();
        let junk = tempfile::tempdir().unwrap();
        let root = junk.path().join("Temp");
        std::fs::create_dir_all(&root).unwrap();
        let st = state(data.path());
        st.settings.lock().unwrap().delete_mode = DeleteMode::Permanent;
        let a = item(&root, "a.tmp", 10);
        let b = item(&root, "b.tmp", 20);
        let c = item(&root, "c.tmp", 40);
        let mut admin = category("admin", vec![c.clone()]);
        admin.admin = true;
        *st.cleanup.lock().unwrap() = Some(CleanupScan {
            id: 42,
            categories: vec![category("t", vec![a.clone(), b.clone()]), admin],
        });

        // A stale scan id is refused and the current scan is kept.
        assert!(matches!(run(&st, 41, &[]), Err(AppError::Stale(_))));
        assert!(st.cleanup.lock().unwrap().is_some());

        let sel = vec![
            Selection {
                id: "t".into(),
                skip: vec![b.path.clone()],
            },
            Selection {
                id: "admin".into(),
                skip: vec![],
            },
        ];
        let report = run(&st, 42, &sel).unwrap();
        assert_eq!(report.removed_items, 1);
        assert_eq!(report.removed_bytes, 10);
        assert!(!report.undo_available, "permanent delete has no undo");
        assert!(!std::path::Path::new(&a.path).exists());
        assert!(
            std::path::Path::new(&b.path).exists(),
            "unticked item stays"
        );
        assert!(
            std::path::Path::new(&c.path).exists(),
            "admin categories are refused"
        );

        // The scan is used up: running again needs a new scan.
        assert!(matches!(run(&st, 42, &sel), Err(AppError::Stale(_))));
        let history = crate::store::read_json::<Vec<Entry>>(&data.path().join("activity.json"))
            .unwrap()
            .unwrap();
        assert_eq!(history[0].kind, activity::Kind::Cleanup);
        assert_eq!(history[0].bytes, 10);
    }
}
