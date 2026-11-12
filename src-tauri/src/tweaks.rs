//! Privacy & Tweaks: list the catalog with each tweak's current state, apply a batch
//! (restore point first when it changes the system), undo one tweak.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use smd_core::tweaks::{MachineUndo, RegPrevious, Tweak, TweakState, builtin_tweaks};
use smd_win::helper::{Op, RestorePoint};
use tauri::{AppHandle, Manager};

use crate::activity::{self, Entry, Undo, UndoData};
use crate::error::{AppError, AppResult};
use crate::state::AppState;
use crate::store::{self, now_ms};

const UNDO_FILE: &str = "tweaks-undo.json";

/// What SMD changed for one tweak, kept until it is undone.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TweakRecord {
    pub activity_id: u64,
    pub time_ms: i64,
    #[serde(default)]
    pub user: Vec<RegPrevious>,
    #[serde(default)]
    pub machine: Option<MachineUndo>,
    /// Store apps removed (package family names, for the Store link).
    #[serde(default)]
    pub appx_families: Vec<String>,
}

fn load_records(state: &AppState) -> HashMap<String, TweakRecord> {
    store::read_json(&state.data_dir.join(UNDO_FILE))
        .ok()
        .flatten()
        .unwrap_or_default()
}

fn save_records(state: &AppState, r: &HashMap<String, TweakRecord>) {
    if let Err(e) = store::write_json(&state.data_dir.join(UNDO_FILE), r) {
        log::warn!("could not save tweak undo data: {e}");
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TweakView {
    #[serde(flatten)]
    pub tweak: Tweak,
    pub state: TweakState,
    pub needs_admin: bool,
    /// Set by Scrub My Disks (can be undone here), with the time.
    pub applied_ms: Option<i64>,
    /// A removed app can be reinstalled from the Microsoft Store.
    pub reinstall: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TweaksView {
    pub tweaks: Vec<TweakView>,
    /// An admin session is open (no new UAC prompt for the next machine-wide tweak).
    pub admin_session: bool,
}

fn view(state: &AppState) -> TweaksView {
    let records = load_records(state);
    let appx = smd_win::tweaks::installed_appx().ok();
    let tweaks = builtin_tweaks()
        .into_iter()
        .map(|t| {
            let st = match &t.appx {
                Some(name) => match &appx {
                    Some(set) if set.contains(&name.to_lowercase()) => TweakState::NotApplied,
                    Some(_) => TweakState::NotInstalled,
                    None => TweakState::Unknown,
                },
                None => smd_win::tweaks::state(&t),
            };
            let rec = records.get(&t.id);
            TweakView {
                needs_admin: t.needs_admin(),
                applied_ms: rec.map(|r| r.time_ms),
                reinstall: rec.is_some_and(|r| !r.appx_families.is_empty()),
                state: st,
                tweak: t,
            }
        })
        .collect();
    TweaksView {
        tweaks,
        admin_session: crate::helper::is_running(state),
    }
}

async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> AppResult<T> + Send + 'static,
) -> AppResult<T> {
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
}

#[tauri::command]
pub async fn tweaks_list(app: AppHandle) -> AppResult<TweaksView> {
    blocking(move || Ok(view(&app.state::<AppState>()))).await
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TweakFailure {
    pub id: String,
    pub message: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyReport {
    /// `None` when the batch only changed your account's settings (no restore point needed).
    pub restore_point: Option<RestorePoint>,
    /// No restore point could be made: nothing was changed; ask before going on without one.
    pub needs_confirm: bool,
    pub applied: Vec<String>,
    pub failed: Vec<TweakFailure>,
    pub restart_explorer: bool,
    pub restart_windows: bool,
    pub activity_id: Option<u64>,
    pub view: TweaksView,
}

/// Creates the restore point that covers a batch of system changes.
pub fn restore_point(state: &AppState, enable_first: bool) -> AppResult<RestorePoint> {
    if enable_first {
        crate::helper::call::<bool>(state, &Op::EnableRestore)?;
    }
    let rp: RestorePoint = crate::helper::call(
        state,
        &Op::RestorePoint {
            description: "Scrub My Disks: before tweaks".into(),
        },
    )?;
    if let RestorePoint::Created { .. } = rp {
        let mut e = Entry::new(state.next_id(), activity::Kind::RestorePoint);
        e.items = 1;
        activity::record(state, e);
    }
    log::info!("restore point: {rp:?}");
    Ok(rp)
}

fn apply_one(state: &AppState, t: &Tweak, activity_id: u64) -> AppResult<TweakRecord> {
    let mut rec = TweakRecord {
        activity_id,
        time_ms: now_ms(),
        ..Default::default()
    };
    if let Some(name) = &t.appx {
        rec.appx_families = smd_win::tweaks::remove_appx(name)?;
        return Ok(rec);
    }
    rec.user = smd_win::tweaks::apply_user(t)?;
    if t.needs_admin() {
        match crate::helper::call::<MachineUndo>(state, &Op::TweakApply { id: t.id.clone() }) {
            Ok(m) => rec.machine = Some(m),
            Err(e) => {
                let _ = smd_win::tweaks::revert_registry(&rec.user);
                return Err(e);
            }
        }
    }
    Ok(rec)
}

pub fn apply(
    state: &AppState,
    ids: &[String],
    without_restore_point: bool,
    enable_restore: bool,
) -> AppResult<ApplyReport> {
    let all = builtin_tweaks();
    let chosen: Vec<&Tweak> = ids
        .iter()
        .map(|id| {
            all.iter()
                .find(|t| &t.id == id)
                .ok_or_else(|| AppError::NotFound(format!("tweak {id}")))
        })
        .collect::<AppResult<_>>()?;
    let mut report = ApplyReport {
        restore_point: None,
        needs_confirm: false,
        applied: Vec::new(),
        failed: Vec::new(),
        restart_explorer: false,
        restart_windows: false,
        activity_id: None,
        view: TweaksView {
            tweaks: Vec::new(),
            admin_session: false,
        },
    };
    if chosen.iter().any(|t| t.needs_admin()) {
        let rp = restore_point(state, enable_restore)?;
        let ok = matches!(
            rp,
            RestorePoint::Created { .. } | RestorePoint::Recent { .. }
        );
        report.restore_point = Some(rp);
        if !ok && !without_restore_point {
            report.needs_confirm = true;
            report.view = view(state);
            return Ok(report);
        }
    }

    let activity_id = state.next_id();
    let mut records = load_records(state);
    for t in chosen {
        match apply_one(state, t, activity_id) {
            Ok(rec) => {
                report.applied.push(t.id.clone());
                report.restart_explorer |= t.restart_explorer;
                report.restart_windows |= t.restart_windows;
                // Keep the oldest record: undo goes back to how it was before SMD touched it.
                records.entry(t.id.clone()).or_insert(rec);
            }
            Err(e) => {
                log::warn!("tweak {} failed: {e}", t.id);
                report.failed.push(TweakFailure {
                    id: t.id.clone(),
                    message: e.to_string(),
                });
            }
        }
    }
    save_records(state, &records);

    if !report.applied.is_empty() || !report.failed.is_empty() {
        let undo = UndoData {
            tweaks: report.applied.clone(),
            ..Default::default()
        };
        let mut e = Entry::new(activity_id, activity::Kind::TweaksApplied);
        e.items = report.applied.len() as u64;
        e.failed = report.failed.len() as u64;
        e.undo = if !report.applied.is_empty() && activity::save_undo(state, activity_id, &undo) {
            Undo::Available
        } else {
            Undo::None
        };
        activity::record(state, e);
        report.activity_id = Some(activity_id);
    }
    log::info!(
        "tweaks applied: {:?}, failed: {}",
        report.applied,
        report.failed.len()
    );
    report.view = view(state);
    Ok(report)
}

#[tauri::command]
pub async fn tweaks_apply(
    app: AppHandle,
    ids: Vec<String>,
    without_restore_point: bool,
    enable_restore: bool,
) -> AppResult<ApplyReport> {
    blocking(move || {
        let state = app.state::<AppState>();
        let _busy = state.begin("tweaks")?;
        apply(&state, &ids, without_restore_point, enable_restore)
    })
    .await
}

/// Puts one tweak back. Removed apps: opens their Microsoft Store page. Returns true if undone.
pub fn revert(app: &AppHandle, state: &AppState, id: &str) -> AppResult<bool> {
    let mut records = load_records(state);
    let Some(rec) = records.get(id).cloned() else {
        return Ok(false);
    };
    if !rec.appx_families.is_empty() {
        use tauri_plugin_opener::OpenerExt;
        for f in &rec.appx_families {
            let url = format!("ms-windows-store://pdp/?PFN={f}");
            if let Err(e) = app.opener().open_url(&url, None::<&str>) {
                log::warn!("could not open the Store page {url}: {e}");
            }
        }
        // Keep the record until the app is back (the page shows "Reinstall" until then).
        return Ok(false);
    }
    if let Some(m) = &rec.machine {
        crate::helper::call::<bool>(
            state,
            &Op::TweakRevert {
                id: id.to_string(),
                undo: m.clone(),
            },
        )?;
    }
    smd_win::tweaks::revert_registry(&rec.user)?;
    records.remove(id);
    save_records(state, &records);
    Ok(true)
}

/// Forgets the record of a removed app once it is installed again.
fn drop_reinstalled(state: &AppState, v: &TweaksView) {
    let mut records = load_records(state);
    let before = records.len();
    records.retain(|id, r| {
        r.appx_families.is_empty()
            || v.tweaks
                .iter()
                .find(|t| &t.tweak.id == id)
                .is_none_or(|t| t.state != TweakState::NotApplied)
    });
    if records.len() != before {
        save_records(state, &records);
    }
}

#[tauri::command]
pub async fn tweak_undo(app: AppHandle, id: String) -> AppResult<TweaksView> {
    blocking(move || {
        let state = app.state::<AppState>();
        let _busy = state.begin("undo")?;
        if revert(&app, &state, &id)? {
            let mut e = Entry::new(state.next_id(), activity::Kind::Restore);
            e.items = 1;
            e.target = Some(id.clone());
            activity::record(&state, e);
        }
        let v = view(&state);
        drop_reinstalled(&state, &v);
        Ok(view(&state))
    })
    .await
}

#[tauri::command]
pub async fn restart_explorer() -> AppResult<()> {
    blocking(|| Ok(smd_win::tweaks::restart_explorer()?)).await
}

/// Closes the admin session now (otherwise the helper exits after 10 idle minutes).
#[tauri::command]
pub fn admin_session_end(state: tauri::State<'_, AppState>) -> AppResult<()> {
    crate::helper::stop(&state);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_round_trip() {
        let mut m = HashMap::new();
        m.insert(
            "advertising-id".to_string(),
            TweakRecord {
                activity_id: 1,
                time_ms: 2,
                ..Default::default()
            },
        );
        let json = serde_json::to_string(&m).unwrap();
        let back: HashMap<String, TweakRecord> = serde_json::from_str(&json).unwrap();
        assert_eq!(back["advertising-id"].activity_id, 1);
        let old: TweakRecord = serde_json::from_str(r#"{"activityId":1,"timeMs":2}"#).unwrap();
        assert!(old.machine.is_none());
    }
}
