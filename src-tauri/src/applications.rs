//! Applications: list, uninstall with each app's own uninstaller, leftovers review, updates.

use std::collections::HashSet;
use std::time::Duration;

use serde::Serialize;
use smd_core::safety;
use smd_win::apps::{InstalledApp, Leftover, LeftoverKind, UninstallResult};
use smd_win::cleanup::{Failure, Item};
use smd_win::winget::{AppUpdate, UpdateResult, WindowsUpdates};
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager};

use crate::activity::{self, Entry, RegBackup, Undo, UndoData};
use crate::error::{AppError, AppResult};
use crate::state::{AppState, LeftoverScan, lock};

async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> AppResult<T> + Send + 'static,
) -> AppResult<T> {
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
}

#[tauri::command]
pub async fn apps_list(app: AppHandle) -> AppResult<Vec<InstalledApp>> {
    blocking(move || {
        let state = app.state::<AppState>();
        let list = smd_win::apps::list()?;
        *lock(&state.apps)? = list.clone();
        Ok(list)
    })
    .await
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UninstallProgress {
    pub index: u32,
    pub total: u32,
    pub name: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UninstallOutcome {
    pub id: String,
    pub name: String,
    pub result: Option<UninstallResult>,
    pub error: Option<String>,
}

/// Runs the official uninstallers one after the other. Only apps from the last list.
#[tauri::command]
pub async fn apps_uninstall(
    app: AppHandle,
    ids: Vec<String>,
    on_progress: Channel<UninstallProgress>,
) -> AppResult<Vec<UninstallOutcome>> {
    blocking(move || {
        let state = app.state::<AppState>();
        let _busy = state.begin_apps("an uninstall")?;
        let known = lock(&state.apps)?.clone();
        let mut out = Vec::new();
        let total = ids.len() as u32;
        for (i, id) in ids.iter().enumerate() {
            let Some(target) = known.iter().find(|a| &a.id == id) else {
                return Err(AppError::Stale(format!(
                    "{id} is not in the current list; refresh it"
                )));
            };
            let _ = on_progress.send(UninstallProgress {
                index: i as u32 + 1,
                total,
                name: target.name.clone(),
            });
            let res = smd_win::apps::uninstall(target);
            let mut e = Entry::new(state.next_id(), activity::Kind::Uninstall);
            e.target = Some(target.name.clone());
            match &res {
                Ok(r) => {
                    e.items = u64::from(!r.still_installed);
                    e.failed = u64::from(r.still_installed);
                }
                Err(_) => e.failed = 1,
            }
            activity::record(&state, e);
            out.push(UninstallOutcome {
                id: id.clone(),
                name: target.name.clone(),
                error: res.as_ref().err().map(|e| e.to_string()),
                result: res.ok(),
            });
        }
        Ok(out)
    })
    .await
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LeftoversView {
    pub scan_id: u64,
    pub app_name: String,
    pub items: Vec<Leftover>,
}

/// Looks for folders and registry keys left by an app that is no longer installed. Read-only.
#[tauri::command]
pub async fn apps_leftovers(app: AppHandle, id: String) -> AppResult<LeftoversView> {
    blocking(move || {
        let state = app.state::<AppState>();
        let target = lock(&state.apps)?
            .iter()
            .find(|a| a.id == id)
            .cloned()
            .ok_or_else(|| AppError::Stale(format!("{id} is not in the current list")))?;
        if smd_win::apps::still_registered(&target.id) {
            return Err(AppError::Stale(format!(
                "{} is still installed",
                target.name
            )));
        }
        let items = smd_win::apps::find_leftovers(&target);
        let scan_id = state.next_id();
        *lock(&state.leftovers)? = Some(LeftoverScan {
            id: scan_id,
            app_name: target.name.clone(),
            items: items.clone(),
            app: target.clone(),
        });
        Ok(LeftoversView {
            scan_id,
            app_name: target.name,
            items,
        })
    })
    .await
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LeftoversReport {
    pub removed_bytes: u64,
    pub removed_items: u64,
    pub failures: Vec<Failure>,
    pub undo_available: bool,
    pub quarantine: Option<String>,
    /// Program Files / ProgramData folders removed by the admin helper (to the Recycle Bin).
    pub admin_bytes: u64,
}

/// Removes the leftovers the user kept ticked: folders to the Recycle Bin, registry keys after a backup.
#[tauri::command]
pub async fn apps_remove_leftovers(
    app: AppHandle,
    scan_id: u64,
    paths: Vec<String>,
) -> AppResult<LeftoversReport> {
    blocking(move || {
        let state = app.state::<AppState>();
        let _busy = state.begin("a leftovers cleanup")?;
        let scan = lock(&state.leftovers)?
            .take()
            .ok_or_else(|| AppError::Stale("no leftovers scan; scan again".into()))?;
        if scan.id != scan_id {
            return Err(AppError::Stale(
                "these results were replaced; scan again".into(),
            ));
        }
        let wanted: HashSet<String> = paths.iter().map(|p| safety::normalize(p)).collect();
        let chosen: Vec<&Leftover> = scan
            .items
            .iter()
            .filter(|l| wanted.contains(&safety::normalize(&l.path)) && !l.needs_admin)
            .collect();
        let admin_paths: Vec<String> = scan
            .items
            .iter()
            .filter(|l| {
                l.needs_admin
                    && l.kind == LeftoverKind::Folder
                    && wanted.contains(&safety::normalize(&l.path))
            })
            .map(|l| l.path.clone())
            .collect();
        let id = state.next_id();

        let folders: Vec<Item> = chosen
            .iter()
            .filter(|l| l.kind == LeftoverKind::Folder)
            .map(|l| Item {
                path: l.path.clone(),
                name: l.path.clone(),
                bytes: l.bytes,
                files: 0,
                modified_ms: None,
                is_dir: true,
                root: l.root.clone(),
                item_is_root: false,
            })
            .collect();
        let removal = crate::removal::remove(&state, id, &folders, false);
        let outcome = removal.outcome;
        let mut failures = outcome.failed;

        let mut registry = Vec::new();
        let backups = activity::undo_files_dir(&state, id);
        for (n, l) in chosen
            .iter()
            .filter(|l| l.kind == LeftoverKind::RegistryKey)
            .enumerate()
        {
            let file = backups.join(format!("{n}.reg"));
            match smd_win::apps::backup_and_delete_key(&l.path, &file) {
                Ok(()) => registry.push(RegBackup {
                    key: l.path.clone(),
                    file,
                }),
                Err(e) => {
                    log::warn!("could not remove {}: {e}", l.path);
                    failures.push(Failure {
                        path: l.path.clone(),
                        reason: smd_win::cleanup::FailReason::InUse,
                    });
                }
            }
        }

        let mut admin_bytes = 0;
        let mut admin_items = 0;
        if !admin_paths.is_empty() {
            let o: smd_win::cleanup::Outcome = crate::helper::call(
                &state,
                &smd_win::helper::Op::LeftoversRemove {
                    app: scan.app.clone(),
                    paths: admin_paths,
                },
            )?;
            admin_bytes = o.removed.iter().map(|r| r.1).sum::<u64>();
            admin_items = o.removed.len();
            failures.extend(o.failed);
        }

        let removed_items = (outcome.removed.len() + registry.len() + admin_items) as u64;
        let removed_bytes = outcome.removed.iter().map(|r| r.1).sum::<u64>() + admin_bytes;
        let undo = UndoData {
            registry,
            ..removal.undo
        };
        let undo_available =
            removed_items > admin_items as u64 && activity::save_undo(&state, id, &undo);
        let mut e = Entry::new(id, activity::Kind::LeftoversRemoved);
        e.target = Some(scan.app_name);
        e.bytes = removed_bytes;
        e.items = removed_items;
        e.failed = failures.len() as u64;
        e.undo = if undo_available {
            Undo::Available
        } else {
            Undo::None
        };
        activity::record(&state, e);
        Ok(LeftoversReport {
            removed_bytes,
            removed_items,
            failures,
            undo_available,
            quarantine: removal.quarantine,
            admin_bytes,
        })
    })
    .await
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdatesStatus {
    /// The WinGet PowerShell module is installed.
    pub module_installed: bool,
}

#[tauri::command]
pub async fn updates_status() -> AppResult<UpdatesStatus> {
    blocking(|| {
        Ok(UpdatesStatus {
            module_installed: smd_win::winget::module_installed()?,
        })
    })
    .await
}

/// Downloads the WinGet PowerShell module from the PowerShell Gallery (network, current user only).
#[tauri::command]
pub async fn updates_setup(app: AppHandle) -> AppResult<UpdatesStatus> {
    blocking(move || {
        let state = app.state::<AppState>();
        let _busy = state.begin_apps("the updates setup")?;
        log::info!("installing the Microsoft.WinGet.Client module for the current user (network)");
        let ok = smd_win::winget::install_module()?;
        let mut e = Entry::new(state.next_id(), activity::Kind::UpdatesSetup);
        e.items = u64::from(ok);
        e.failed = u64::from(!ok);
        activity::record(&state, e);
        Ok(UpdatesStatus {
            module_installed: ok,
        })
    })
    .await
}

#[tauri::command]
pub async fn updates_list() -> AppResult<Vec<AppUpdate>> {
    blocking(|| Ok(smd_win::winget::list_updates(Duration::from_secs(180))?)).await
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateProgress {
    pub index: u32,
    pub total: u32,
    pub id: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateOutcome {
    pub id: String,
    pub result: Option<UpdateResult>,
    pub error: Option<String>,
}

/// Updates packages one at a time with their own installers.
#[tauri::command]
pub async fn updates_apply(
    app: AppHandle,
    ids: Vec<String>,
    on_progress: Channel<UpdateProgress>,
) -> AppResult<Vec<UpdateOutcome>> {
    blocking(move || {
        let state = app.state::<AppState>();
        let _busy = state.begin_apps("an update")?;
        let total = ids.len() as u32;
        let mut out = Vec::new();
        for (i, id) in ids.iter().enumerate() {
            let _ = on_progress.send(UpdateProgress {
                index: i as u32 + 1,
                total,
                id: id.clone(),
            });
            let res = smd_win::winget::update(id);
            let ok = res
                .as_ref()
                .is_ok_and(|r| r.status.eq_ignore_ascii_case("ok"));
            let mut e = Entry::new(state.next_id(), activity::Kind::AppUpdate);
            e.target = Some(id.clone());
            e.items = u64::from(ok);
            e.failed = u64::from(!ok);
            activity::record(&state, e);
            out.push(UpdateOutcome {
                id: id.clone(),
                error: res.as_ref().err().map(|e| e.to_string()),
                result: res.ok(),
            });
        }
        Ok(out)
    })
    .await
}

#[tauri::command]
pub async fn windows_updates() -> AppResult<WindowsUpdates> {
    blocking(|| Ok(smd_win::winget::windows_updates(Duration::from_secs(60))?)).await
}
