//! Protection: Microsoft Defender status, scans and signatures; firewall; app permissions.

use std::time::{Duration, Instant};

use serde::Serialize;
use smd_win::protection::{
    Capability, CapabilityState, DefenderStatus, FirewallProfile, ScanKind, Threat,
};
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager};

use crate::activity::{self, Entry, PermissionUndo, Undo, UndoData};
use crate::error::{AppError, AppResult};
use crate::state::{AppState, lock};

async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> AppResult<T> + Send + 'static,
) -> AppResult<T> {
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProtectionView {
    /// `None` when Defender could not be read (removed, or managed by another antivirus).
    pub defender: Option<DefenderStatus>,
    pub threats: Option<Vec<Threat>>,
    pub firewall: Option<Vec<FirewallProfile>>,
    pub permissions: Vec<CapabilityState>,
}

#[tauri::command]
pub async fn protection_status() -> AppResult<ProtectionView> {
    blocking(|| {
        let (defender, threats, firewall) = std::thread::scope(|s| {
            let d = s.spawn(smd_win::protection::defender_status);
            let t = s.spawn(smd_win::protection::threats);
            let f = smd_win::protection::firewall();
            (
                d.join().ok().and_then(|r| r.ok()),
                t.join().ok().and_then(|r| r.ok()),
                f.ok(),
            )
        });
        Ok(ProtectionView {
            defender,
            threats,
            firewall,
            permissions: smd_win::protection::permissions(),
        })
    })
    .await
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanTick {
    pub elapsed_ms: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanResult {
    pub cancelled: bool,
    /// Defender found (and handled) threats during this scan.
    pub threats_found: bool,
    pub exit_code: Option<i32>,
    pub duration_ms: u64,
    pub threats: Option<Vec<Threat>>,
}

/// Runs a Defender scan with Defender's own tool. Defender decides what to do with anything it finds.
#[tauri::command]
pub async fn protection_scan(
    app: AppHandle,
    kind: ScanKind,
    on_progress: Channel<ScanTick>,
) -> AppResult<ScanResult> {
    blocking(move || {
        let state = app.state::<AppState>();
        let started = Instant::now();
        {
            // A full scan can take an hour: it has its own slot instead of blocking the other tasks.
            let mut slot = lock(&state.defender_scan)?;
            if slot.is_some() {
                return Err(AppError::Busy("a Defender scan is already running".into()));
            }
            *slot = Some(smd_win::protection::start_scan(kind)?);
        }
        log::info!("Defender {kind:?} scan started");
        let status = loop {
            std::thread::sleep(Duration::from_millis(500));
            let mut slot = lock(&state.defender_scan)?;
            let Some(child) = slot.as_mut() else {
                break None;
            };
            if let Some(s) = child.try_wait()? {
                *slot = None;
                break Some(s);
            }
            drop(slot);
            let _ = on_progress.send(ScanTick {
                elapsed_ms: started.elapsed().as_millis() as u64,
            });
        };
        let cancelled = status.is_none();
        let exit_code = status.and_then(|s| s.code());
        let threats_found = exit_code == Some(2);
        let mut e = Entry::new(state.next_id(), activity::Kind::DefenderScan);
        e.target = Some(format!("{kind:?}").to_lowercase());
        e.items = u64::from(threats_found);
        e.failed = u64::from(cancelled || exit_code.is_some_and(|c| c != 0 && c != 2));
        activity::record(&state, e);
        log::info!("Defender scan finished: exit {exit_code:?}, cancelled {cancelled}");
        Ok(ScanResult {
            cancelled,
            threats_found,
            exit_code,
            duration_ms: started.elapsed().as_millis() as u64,
            threats: smd_win::protection::threats().ok(),
        })
    })
    .await
}

#[tauri::command]
pub fn protection_cancel_scan(state: tauri::State<'_, AppState>) -> AppResult<()> {
    if let Some(mut child) = lock(&state.defender_scan)?.take() {
        let _ = child.kill();
        let _ = child.wait();
    }
    Ok(())
}

/// Asks Defender to download its latest signatures (network: Microsoft Update).
#[tauri::command]
pub async fn protection_update_signatures(app: AppHandle) -> AppResult<Option<DefenderStatus>> {
    blocking(move || {
        let state = app.state::<AppState>();
        let ok = smd_win::protection::update_signatures()?;
        let mut e = Entry::new(state.next_id(), activity::Kind::SignatureUpdate);
        e.items = u64::from(ok);
        e.failed = u64::from(!ok);
        activity::record(&state, e);
        Ok(smd_win::protection::defender_status().ok())
    })
    .await
}

/// Allows or denies one Store app's access to the camera, microphone or location. With undo.
#[tauri::command]
pub async fn permission_set(
    app: AppHandle,
    capability: Capability,
    id: String,
    allowed: bool,
) -> AppResult<Vec<CapabilityState>> {
    blocking(move || {
        let state = app.state::<AppState>();
        let was =
            smd_win::protection::set_permission(capability, &id, allowed).map_err(|e| {
                match e.kind() {
                    std::io::ErrorKind::NotFound => AppError::NotFound(id.clone()),
                    std::io::ErrorKind::InvalidInput => AppError::AccessDenied(e.to_string()),
                    _ => AppError::Io(e),
                }
            })?;
        let entry_id = state.next_id();
        let undo = UndoData {
            permission: Some(PermissionUndo {
                capability,
                id: id.clone(),
                was_allowed: was,
            }),
            ..Default::default()
        };
        let mut e = Entry::new(entry_id, activity::Kind::PermissionChange);
        e.target = Some(format!(
            "{}|{}",
            capability.key(),
            smd_win::protection::app_label(&id, true)
        ));
        e.enabled = Some(allowed);
        e.items = 1;
        e.undo = if was != allowed && activity::save_undo(&state, entry_id, &undo) {
            Undo::Available
        } else {
            Undo::None
        };
        activity::record(&state, e);
        Ok(smd_win::protection::permissions())
    })
    .await
}
