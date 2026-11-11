//! Space Lens: scan a drive or folder (read-only, streamed, cancellable), then browse it.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use serde::Serialize;
use smd_core::space::{Arena, Crumb, ExtStat, ViewNode, top_extensions};
use smd_win::system::DriveInfo;
use smd_win::walk::{Progress, WalkOptions, walk};
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager};
use tauri_plugin_dialog::DialogExt;

use crate::activity::{self, Entry};
use crate::error::{AppError, AppResult};
use crate::state::{AppState, SpaceScan, lock};
use crate::store::now_ms;

const VIEW_DEPTH: u8 = 2;
const VIEW_MAX_CHILDREN: usize = 60;
const TOP_EXTENSIONS: usize = 12;
const PROGRESS_EVERY: Duration = Duration::from_millis(150);

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpaceProgress {
    pub files: u64,
    pub dirs: u64,
    pub bytes: u64,
    pub denied: u64,
    pub current: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpaceView {
    pub scan_id: u64,
    pub node: ViewNode,
    pub crumbs: Vec<Crumb>,
    /// Full path of the node, if it is a real file or folder.
    pub path: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpaceSummary {
    pub cancelled: bool,
    pub root_path: String,
    pub view: Option<SpaceView>,
    pub extensions: Vec<ExtStat>,
    pub files: u64,
    pub dirs: u64,
    pub bytes: u64,
    /// Folders Windows did not let us read.
    pub denied: u64,
    pub duration_ms: u64,
    /// Total and free space when the target is a whole drive.
    pub drive: Option<DriveInfo>,
}

#[tauri::command]
pub fn drives_list() -> AppResult<Vec<DriveInfo>> {
    Ok(smd_win::system::drives()
        .into_iter()
        .filter(|d| d.total_bytes > 0)
        .collect())
}

/// Opens the Windows folder picker. `None` if the user cancels.
#[tauri::command]
pub async fn pick_folder(app: AppHandle) -> AppResult<Option<String>> {
    tauri::async_runtime::spawn_blocking(move || {
        let picked = app.dialog().file().blocking_pick_folder();
        Ok(picked
            .and_then(|p| p.into_path().ok())
            .map(|p| p.to_string_lossy().into_owned()))
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))?
}

fn view(scan_id: u64, arena: &Arena, node: u32) -> AppResult<SpaceView> {
    let v = arena
        .view(node, VIEW_DEPTH, VIEW_MAX_CHILDREN)
        .ok_or_else(|| AppError::NotFound(format!("node {node}")))?;
    Ok(SpaceView {
        scan_id,
        node: v,
        crumbs: arena.crumbs(node),
        path: arena.path_of(node),
    })
}

pub fn scan(
    state: &AppState,
    path: &Path,
    on_progress: &Channel<SpaceProgress>,
) -> AppResult<SpaceSummary> {
    let meta =
        std::fs::metadata(path).map_err(|_| AppError::NotFound(path.display().to_string()))?;
    if !meta.is_dir() {
        return Err(AppError::NotFound(format!(
            "{} is not a folder",
            path.display()
        )));
    }
    let started = now_ms();
    state.space_cancel.store(false, Ordering::Relaxed);
    let progress = Progress::default();
    let finished = AtomicBool::new(false);
    let send = || {
        let _ = on_progress.send(SpaceProgress {
            files: progress.files.load(Ordering::Relaxed),
            dirs: progress.dirs.load(Ordering::Relaxed),
            bytes: progress.bytes.load(Ordering::Relaxed),
            denied: progress.denied.load(Ordering::Relaxed),
            current: progress.current(),
        });
    };

    let result = std::thread::scope(|s| {
        s.spawn(|| {
            while !finished.load(Ordering::Relaxed) {
                std::thread::sleep(PROGRESS_EVERY);
                send();
            }
        });
        let r = walk(path, WalkOptions::default(), &progress, &state.space_cancel);
        finished.store(true, Ordering::Relaxed);
        r
    });
    send();

    let root_path = path.to_string_lossy().into_owned();
    let drive = smd_win::system::drives()
        .into_iter()
        .find(|d| smd_core::safety::same_path(&d.root, &root_path));
    let mut summary = SpaceSummary {
        cancelled: result.cancelled,
        root_path: root_path.clone(),
        view: None,
        extensions: Vec::new(),
        files: progress.files.load(Ordering::Relaxed),
        dirs: progress.dirs.load(Ordering::Relaxed),
        bytes: result.tree.size,
        denied: progress.denied.load(Ordering::Relaxed),
        duration_ms: (now_ms() - started).max(0) as u64,
        drive,
    };
    if result.cancelled {
        log::info!("space lens scan of {root_path} cancelled");
        return Ok(summary);
    }

    let id = state.next_id();
    let arena = Arena::from_tree(root_path.clone(), result.tree);
    summary.view = Some(view(id, &arena, 0)?);
    summary.extensions = top_extensions(result.extensions, TOP_EXTENSIONS);
    *lock(&state.space)? = Some(SpaceScan { id, arena });

    let mut e = Entry::new(state.next_id(), activity::Kind::SpaceLensScan);
    e.bytes = summary.bytes;
    e.items = summary.files;
    e.target = Some(root_path.clone());
    activity::record(state, e);
    log::info!(
        "space lens {id}: {root_path}, {} files, {} bytes, {} ms",
        summary.files,
        summary.bytes,
        summary.duration_ms
    );
    Ok(summary)
}

#[tauri::command]
pub async fn space_scan(
    app: AppHandle,
    path: String,
    on_progress: Channel<SpaceProgress>,
) -> AppResult<SpaceSummary> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let _busy = state.begin_space()?;
        // Free the previous tree before building a new one.
        *lock(&state.space)? = None;
        scan(&state, &PathBuf::from(path), &on_progress)
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))?
}

#[tauri::command]
pub fn space_cancel(state: tauri::State<'_, AppState>) -> AppResult<()> {
    state.space_cancel.store(true, Ordering::Relaxed);
    Ok(())
}

fn with_scan<T>(
    state: &AppState,
    scan_id: u64,
    f: impl FnOnce(&SpaceScan) -> AppResult<T>,
) -> AppResult<T> {
    let guard = lock(&state.space)?;
    match guard.as_ref() {
        Some(scan) if scan.id == scan_id => f(scan),
        _ => Err(AppError::Stale(format!(
            "space scan {scan_id} is no longer loaded"
        ))),
    }
}

#[tauri::command]
pub fn space_view(
    state: tauri::State<'_, AppState>,
    scan_id: u64,
    node_id: u32,
) -> AppResult<SpaceView> {
    with_scan(&state, scan_id, |s| view(s.id, &s.arena, node_id))
}

/// Opens Explorer with the item selected.
#[tauri::command]
pub fn space_reveal(
    state: tauri::State<'_, AppState>,
    scan_id: u64,
    node_id: u32,
) -> AppResult<()> {
    let path = with_scan(&state, scan_id, |s| {
        s.arena
            .path_of(node_id)
            .ok_or_else(|| AppError::NotFound(format!("node {node_id}")))
    })?;
    tauri_plugin_opener::reveal_item_in_dir(&path).map_err(|e| AppError::Internal(e.to_string()))
}
