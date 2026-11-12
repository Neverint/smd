//! Drive Organizer: audit the drive layout, plan moves with a dry run, run them with a
//! journal, and manage the quarantine and scratch folders on the disposable drive.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use serde::Serialize;
use smd_core::health::{DiskHealth, Status};
use smd_core::organizer::{
    CACHES, Check, Finding, Method, Policy, PolicyNotice, PreflightFacts, Role, Severity,
    bind_policy, drive_speed_mbps, estimate_seconds, layout_status, preflight,
};
use smd_win::disks::PhysicalDisk;
use smd_win::migrate::{Journal, JournalStep, MigrateProgress, StepState, WindowsEffects};
use smd_win::system::DriveInfo;
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager};

use crate::activity::{self, Entry, Undo, UndoData};
use crate::error::{AppError, AppResult};
use crate::state::{AppState, lock};
use crate::store::{self, now_ms};

const POLICY_FILE: &str = "drive-policy.json";
const MIGRATIONS: &str = "migrations";

async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> AppResult<T> + Send + 'static,
) -> AppResult<T> {
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
}

/// The saved policy, or the shipped default on first run.
pub fn load_policy(state: &AppState) -> Policy {
    store::read_json::<Policy>(&state.data_dir.join(POLICY_FILE))
        .ok()
        .flatten()
        .filter(|p| p.validate().is_ok())
        .unwrap_or_else(Policy::default_policy)
}

fn migrations_dir(state: &AppState) -> PathBuf {
    state.data_dir.join(MIGRATIONS)
}

/// What the audit saw, kept for planning.
pub struct AuditState {
    pub policy: Policy,
    pub drives: Vec<DriveInfo>,
    pub disks: Vec<PhysicalDisk>,
    pub findings: Vec<Finding>,
}

/// A planned migration waiting for the user's go.
pub struct PlanState {
    pub id: u64,
    pub steps: Vec<JournalStep>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DriveView {
    pub letter: String,
    pub label: String,
    pub role: Role,
    pub total_bytes: u64,
    pub free_bytes: u64,
    pub disk_name: Option<String>,
    pub media_type: Option<String>,
    pub bus_type: Option<String>,
    pub serial: Option<String>,
    pub health: Option<DiskHealth>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct JournalSummary {
    pub id: u64,
    pub created_ms: i64,
    pub steps: usize,
    pub done: usize,
    pub failed: usize,
    pub bytes: u64,
}

fn summarize(j: &Journal) -> JournalSummary {
    JournalSummary {
        id: j.id,
        created_ms: j.created_ms,
        steps: j.steps.len(),
        done: j
            .steps
            .iter()
            .filter(|s| s.state == StepState::Done)
            .count(),
        failed: j
            .steps
            .iter()
            .filter(|s| s.state == StepState::Failed)
            .count(),
        bytes: j.steps.iter().map(|s| s.bytes).sum(),
    }
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct QuarantineSummary {
    pub drive: Option<String>,
    /// Quarantine is on and the drive is there and healthy.
    pub active: bool,
    pub bytes: u64,
    pub batches: usize,
    /// Bytes older than the keep period.
    pub old_bytes: u64,
    pub purge_consent: Option<bool>,
    pub days: u32,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AuditView {
    pub audit_id: u64,
    pub policy: Policy,
    pub drives: Vec<DriveView>,
    pub notices: Vec<PolicyNotice>,
    pub findings: Vec<Finding>,
    pub status: Status,
    pub misplaced_bytes: u64,
    pub games: usize,
    pub unfinished: Vec<JournalSummary>,
    pub quarantine: QuarantineSummary,
    pub duration_ms: u64,
}

pub fn quarantine_summary(state: &AppState, policy: &Policy) -> QuarantineSummary {
    let settings = lock(&state.settings).map(|s| s.clone()).unwrap_or_default();
    let drive = policy.drive_for(Role::Disposable).map(str::to_string);
    let root = drive.as_deref().map(smd_win::quarantine::root_for);
    let batches = root
        .as_deref()
        .map(smd_win::quarantine::list)
        .unwrap_or_default();
    let cutoff = now_ms() - i64::from(settings.quarantine_days) * 86_400_000;
    QuarantineSummary {
        active: crate::removal::quarantine_root(state).is_some(),
        drive,
        bytes: batches.iter().map(smd_win::quarantine::Batch::bytes).sum(),
        batches: batches.len(),
        old_bytes: batches
            .iter()
            .filter(|b| b.created_ms < cutoff)
            .map(smd_win::quarantine::Batch::bytes)
            .sum(),
        purge_consent: settings.purge_consent,
        days: settings.quarantine_days,
    }
}

/// Reads the drives, binds the policy, finds games and misplaced items. Read-only
/// (except saving the policy's volume GUIDs).
pub fn run_audit(state: &AppState) -> AppResult<AuditView> {
    let started = now_ms();
    let drives = smd_win::system::drives();
    let disks = smd_win::disks::physical_disks().unwrap_or_default();
    let seen = smd_win::organizer::seen_volumes(&drives, &disks);
    let (policy, notices) = bind_policy(&load_policy(state), &seen);
    if let Err(e) = store::write_json(&state.data_dir.join(POLICY_FILE), &policy) {
        log::warn!("could not save the drive policy: {e}");
    }
    let games = smd_win::organizer::all_games(&drives);
    let findings = smd_win::organizer::audit(&policy, &drives, &games);
    let system_free = policy
        .drive_for(Role::System)
        .and_then(|l| drives.iter().find(|d| d.letter.eq_ignore_ascii_case(l)))
        .map(DriveInfo::free_percent);
    let unhealthy = disks
        .iter()
        .any(|d| matches!(d.health, DiskHealth::Warning | DiskHealth::Unhealthy));
    let status = layout_status(&findings, system_free, unhealthy);
    let misplaced_bytes = findings
        .iter()
        .filter(|f| f.severity != Severity::Info)
        .map(|f| f.bytes)
        .sum();

    let views = drives
        .iter()
        .filter(|d| d.kind == smd_win::system::DriveKind::Fixed)
        .map(|d| {
            let disk = disks
                .iter()
                .find(|k| k.letters.iter().any(|l| l.eq_ignore_ascii_case(&d.letter)));
            DriveView {
                letter: d.letter.clone(),
                label: d.label.clone(),
                role: policy.role_of(&d.letter),
                total_bytes: d.total_bytes,
                free_bytes: d.free_bytes,
                disk_name: disk.map(|k| k.name.clone()),
                media_type: disk.map(|k| k.media_type.clone()),
                bus_type: disk.map(|k| k.bus_type.clone()),
                serial: disk.map(|k| k.serial.clone()),
                health: disk.map(|k| k.health),
            }
        })
        .collect();
    let id = state.next_id();
    let unfinished = smd_win::migrate::unfinished(&migrations_dir(state))
        .iter()
        .map(|(_, j)| summarize(j))
        .collect();
    let quarantine = quarantine_summary(state, &policy);
    *lock(&state.organizer_audit)? = Some(AuditState {
        policy: policy.clone(),
        drives,
        disks,
        findings: findings.clone(),
    });
    Ok(AuditView {
        audit_id: id,
        policy,
        drives: views,
        notices,
        findings,
        status,
        misplaced_bytes,
        games: games.len(),
        unfinished,
        quarantine,
        duration_ms: (now_ms() - started).max(0) as u64,
    })
}

#[tauri::command]
pub async fn organizer_audit(app: AppHandle) -> AppResult<AuditView> {
    blocking(move || run_audit(&app.state::<AppState>())).await
}

#[tauri::command]
pub async fn organizer_policy_set(app: AppHandle, policy: Policy) -> AppResult<AuditView> {
    blocking(move || {
        let state = app.state::<AppState>();
        policy.validate().map_err(AppError::Internal)?;
        store::write_json(&state.data_dir.join(POLICY_FILE), &policy)?;
        log::info!("drive policy saved");
        run_audit(&state)
    })
    .await
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanStep {
    pub finding: Finding,
    /// Where it goes (for steps the app runs).
    pub target: Option<String>,
    pub est_seconds: u64,
    pub checks: Vec<Check>,
    /// The app runs this step (copy-verify). Launcher steps are guided instead.
    pub runs_in_app: bool,
    /// Every check passed.
    pub ready: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanView {
    pub plan_id: u64,
    pub steps: Vec<PlanStep>,
    pub ready_bytes: u64,
    pub est_seconds: u64,
}

fn files_for_lock_check(path: &Path, limit: usize) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![path.to_path_buf()];
    if path.is_file() {
        return vec![path.to_path_buf()];
    }
    while let Some(d) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&d) else {
            continue;
        };
        for e in entries.flatten() {
            if out.len() >= limit {
                return out;
            }
            match e.file_type() {
                Ok(t) if t.is_symlink() => {}
                Ok(t) if t.is_dir() => stack.push(e.path()),
                Ok(_) => out.push(e.path()),
                Err(_) => {}
            }
        }
    }
    out
}

/// Dry run: order, source, target, size, method, estimated time and every pre-check. Changes nothing.
#[tauri::command]
pub async fn organizer_plan(app: AppHandle, ids: Vec<String>) -> AppResult<PlanView> {
    blocking(move || {
        let state = app.state::<AppState>();
        let audit = lock(&state.organizer_audit)?;
        let audit = audit
            .as_ref()
            .ok_or_else(|| AppError::Stale("run the audit first".into()))?;
        let running = smd_win::system::running_process_names();
        let on_battery = smd_win::system::on_battery();
        let disk_of = |letter: &str| {
            audit
                .disks
                .iter()
                .find(|k| k.letters.iter().any(|l| l.eq_ignore_ascii_case(letter)))
        };
        let healthy = |letter: &str| {
            disk_of(letter)
                .is_none_or(|k| !matches!(k.health, DiskHealth::Warning | DiskHealth::Unhealthy))
        };
        let speed = |letter: &str| {
            disk_of(letter).map_or(120, |k| drive_speed_mbps(&k.media_type, &k.bus_type))
        };
        // Free space left on each target after the steps before it.
        let mut free: HashMap<String, u64> = audit
            .drives
            .iter()
            .map(|d| (d.letter.to_ascii_uppercase(), d.free_bytes))
            .collect();

        let mut steps = Vec::new();
        let mut journal_steps = Vec::new();
        for id in &ids {
            let Some(f) = audit.findings.iter().find(|f| &f.id == id) else {
                return Err(AppError::Stale(format!(
                    "{id} is not in the current audit; audit again"
                )));
            };
            let target_drive = f
                .target_drive
                .clone()
                .unwrap_or_default()
                .to_ascii_uppercase();
            let target_present = free.contains_key(&target_drive);
            let wanted_role = audit.policy.role_of(&target_drive);
            let runs = f.method.runs_in_app();
            // Only steps the app runs have a target path; launchers choose their own folder.
            let target = (runs && !target_drive.is_empty())
                .then(|| smd_win::organizer::target_path(f, &target_drive));
            let lockables = if runs {
                files_for_lock_check(Path::new(&f.path), 1000)
            } else {
                Vec::new()
            };
            let lock_refs: Vec<&Path> = lockables.iter().map(PathBuf::as_path).collect();
            let procs: Vec<String> = match &f.method {
                Method::RelocateCache { env_var } => CACHES
                    .iter()
                    .filter(|c| c.env_var == env_var.as_str())
                    .flat_map(|c| c.processes.iter())
                    .filter(|p| running.contains(&p.to_lowercase()))
                    .map(|p| p.to_string())
                    .collect(),
                _ => Vec::new(),
            };
            let facts = PreflightFacts {
                target_role_allows: target_present
                    && wanted_role != Role::Disposable
                    && wanted_role != Role::System,
                bytes: f.bytes,
                target_free: free.get(&target_drive).copied().unwrap_or(0),
                source_healthy: healthy(&f.drive),
                target_healthy: target_present && healthy(&target_drive),
                locked_by: if runs {
                    smd_win::locks::locked_by(&lock_refs).unwrap_or_default()
                } else {
                    Vec::new()
                },
                running: procs,
                on_battery,
                target_exists: target.as_deref().is_some_and(|t| Path::new(t).exists()),
            };
            let checks = preflight(&facts);
            let ready = checks.iter().all(|c| c.ok);
            if ready && runs {
                if let Some(v) = free.get_mut(&target_drive) {
                    *v = v.saturating_sub(f.bytes);
                }
                journal_steps.push(JournalStep {
                    finding_id: f.id.clone(),
                    name: f.name.clone(),
                    source: f.path.clone(),
                    target: target.clone().unwrap_or_default(),
                    bytes: f.bytes,
                    env_var: match &f.method {
                        Method::RelocateCache { env_var } => Some(env_var.clone()),
                        _ => None,
                    },
                    env_change: None,
                    state: StepState::Pending,
                    error: None,
                    files: None,
                    skipped_links: 0,
                });
            }
            steps.push(PlanStep {
                est_seconds: estimate_seconds(f.bytes, speed(&f.drive), speed(&target_drive)),
                finding: f.clone(),
                target,
                checks,
                runs_in_app: runs,
                ready,
            });
        }
        let plan_id = state.next_id();
        let ready_bytes = journal_steps.iter().map(|s| s.bytes).sum();
        let est_seconds = steps
            .iter()
            .filter(|s| s.ready && s.runs_in_app)
            .map(|s| s.est_seconds)
            .sum();
        *lock(&state.organizer_plan)? = Some(PlanState {
            id: plan_id,
            steps: journal_steps,
        });
        Ok(PlanView {
            plan_id,
            steps,
            ready_bytes,
            est_seconds,
        })
    })
    .await
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveProgress {
    pub step: u64,
    pub steps: u64,
    pub bytes_done: u64,
    pub bytes_total: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveResult {
    pub journal: JournalSummary,
    pub steps: Vec<JournalStep>,
    pub cancelled: bool,
    pub undo_available: bool,
}

fn execute(
    state: &AppState,
    path: &Path,
    on_progress: &Channel<MoveProgress>,
) -> AppResult<MoveResult> {
    let progress = MigrateProgress::default();
    let finished = AtomicBool::new(false);
    state.organizer_cancel.store(false, Ordering::Relaxed);
    let steps_count = Journal::load(path)?.steps.len() as u64;
    let result = std::thread::scope(|s| {
        s.spawn(|| {
            while !finished.load(Ordering::Relaxed) {
                std::thread::sleep(Duration::from_millis(300));
                let _ = on_progress.send(MoveProgress {
                    step: progress.step.load(Ordering::Relaxed),
                    steps: steps_count,
                    bytes_done: progress.bytes_done.load(Ordering::Relaxed),
                    bytes_total: progress.bytes_total.load(Ordering::Relaxed),
                });
            }
        });
        let r = smd_win::migrate::run(path, &WindowsEffects, &progress, &state.organizer_cancel);
        finished.store(true, Ordering::Relaxed);
        r
    })?;
    let done: Vec<&JournalStep> = result
        .steps
        .iter()
        .filter(|s| s.state == StepState::Done)
        .collect();
    let id = result.id;
    let undo = UndoData {
        migration: Some(path.to_string_lossy().into_owned()),
        ..Default::default()
    };
    let undo_available = !done.is_empty() && activity::save_undo(state, id, &undo);
    if result.finished {
        let mut e = Entry::new(id, activity::Kind::Migration);
        e.bytes = done.iter().map(|s| s.bytes).sum();
        e.items = done.len() as u64;
        e.failed = result
            .steps
            .iter()
            .filter(|s| s.state == StepState::Failed)
            .count() as u64;
        e.undo = if undo_available {
            Undo::Available
        } else {
            Undo::None
        };
        activity::record(state, e);
    }
    log::info!(
        "migration {id}: {} of {} steps done, finished {}",
        done.len(),
        result.steps.len(),
        result.finished
    );
    Ok(MoveResult {
        journal: summarize(&result),
        cancelled: !result.finished,
        undo_available,
        steps: result.steps,
    })
}

/// Runs the ready steps of the plan the user just reviewed (only those it lists).
#[tauri::command]
pub async fn organizer_run(
    app: AppHandle,
    plan_id: u64,
    finding_ids: Vec<String>,
    on_progress: Channel<MoveProgress>,
) -> AppResult<MoveResult> {
    blocking(move || {
        let state = app.state::<AppState>();
        let _busy = state.begin("a migration")?;
        let plan = lock(&state.organizer_plan)?
            .take()
            .ok_or_else(|| AppError::Stale("plan again".into()))?;
        if plan.id != plan_id {
            return Err(AppError::Stale("this plan was replaced; plan again".into()));
        }
        let steps: Vec<JournalStep> = plan
            .steps
            .into_iter()
            .filter(|s| finding_ids.contains(&s.finding_id))
            .collect();
        if steps.is_empty() {
            return Err(AppError::Stale("nothing ready to move".into()));
        }
        let journal = Journal {
            id: state.next_id(),
            created_ms: now_ms(),
            steps,
            finished: false,
        };
        let path = migrations_dir(&state).join(format!("{}.json", journal.id));
        journal.save(&path)?;
        execute(&state, &path, &on_progress)
    })
    .await
}

/// Continues a migration a cancel, crash or reboot interrupted.
#[tauri::command]
pub async fn organizer_resume(
    app: AppHandle,
    journal_id: u64,
    on_progress: Channel<MoveProgress>,
) -> AppResult<MoveResult> {
    blocking(move || {
        let state = app.state::<AppState>();
        let _busy = state.begin("a migration")?;
        let path = migrations_dir(&state).join(format!("{journal_id}.json"));
        if !path.is_file() {
            return Err(AppError::NotFound(format!("migration {journal_id}")));
        }
        execute(&state, &path, &on_progress)
    })
    .await
}

#[tauri::command]
pub fn organizer_cancel(state: tauri::State<'_, AppState>) -> AppResult<()> {
    state.organizer_cancel.store(true, Ordering::Relaxed);
    Ok(())
}

/// Rolls a migration back: originals restored from the Recycle Bin, copies recycled, variables restored.
pub fn undo_migration(journal: &Path) -> AppResult<smd_win::cleanup::RestoreReport> {
    let j = Journal::load(journal)?;
    let mut report = smd_win::cleanup::RestoreReport::default();
    for s in j.steps.iter().rev().filter(|s| s.state == StepState::Done) {
        let r = smd_win::cleanup::restore(std::slice::from_ref(&s.source), j.created_ms / 1000)?;
        if r.restored > 0 {
            report.restored += 1;
            if let Some(env) = &s.env_change {
                let _ = smd_win::env::set_user(&env.name, env.old.as_deref());
            }
            if let Err(e) =
                smd_win::migrate::Effects::recycle(&WindowsEffects, Path::new(&s.target))
            {
                log::warn!("original restored, but the copy {} stays: {e}", s.target);
            }
        } else {
            report.missing += r.missing;
            report.failed += r.failed;
        }
    }
    Ok(report)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuarantineBatchView {
    pub id: u64,
    pub manifest: String,
    pub created_ms: i64,
    pub bytes: u64,
    pub items: usize,
    pub sample: Vec<String>,
}

#[tauri::command]
pub async fn quarantine_list(app: AppHandle) -> AppResult<Vec<QuarantineBatchView>> {
    blocking(move || {
        let state = app.state::<AppState>();
        let policy = load_policy(&state);
        let Some(letter) = policy.drive_for(Role::Disposable) else {
            return Ok(Vec::new());
        };
        Ok(
            smd_win::quarantine::list(&smd_win::quarantine::root_for(letter))
                .into_iter()
                .map(|b| QuarantineBatchView {
                    id: b.id,
                    manifest: b.dir.join("manifest.json").to_string_lossy().into_owned(),
                    created_ms: b.created_ms,
                    bytes: b.bytes(),
                    items: b.items.iter().filter(|i| !i.restored).count(),
                    sample: b
                        .items
                        .iter()
                        .filter(|i| !i.restored)
                        .take(3)
                        .map(|i| i.original.clone())
                        .collect(),
                })
                .collect(),
        )
    })
    .await
}

/// Puts a quarantine batch back. Only manifests inside the quarantine folder.
#[tauri::command]
pub async fn quarantine_restore(
    app: AppHandle,
    manifest: String,
) -> AppResult<smd_win::quarantine::QuarantineRestore> {
    blocking(move || {
        let state = app.state::<AppState>();
        let policy = load_policy(&state);
        let root = policy
            .drive_for(Role::Disposable)
            .map(smd_win::quarantine::root_for)
            .ok_or_else(|| AppError::NotFound("no disposable drive".into()))?;
        if !smd_core::safety::is_inside(&manifest, &root.to_string_lossy())
            || !manifest.to_lowercase().ends_with("manifest.json")
        {
            return Err(AppError::AccessDenied("not a quarantine batch".into()));
        }
        let _busy = state.begin("a restore")?;
        Ok(smd_win::quarantine::restore(Path::new(&manifest))?)
    })
    .await
}

/// Answers "purge old quarantine batches automatically?" and purges now when yes.
#[tauri::command]
pub async fn quarantine_purge_consent(app: AppHandle, allow: bool) -> AppResult<u64> {
    blocking(move || {
        let state = app.state::<AppState>();
        let settings = {
            let mut s = lock(&state.settings)?;
            s.purge_consent = Some(allow);
            s.clone()
        };
        store::write_json(&state.data_dir.join(crate::settings::FILE), &settings)?;
        if allow { Ok(purge_old(&state)) } else { Ok(0) }
    })
    .await
}

/// Deletes quarantine batches older than the keep period for good. Only after consent.
pub fn purge_old(state: &AppState) -> u64 {
    let settings = lock(&state.settings).map(|s| s.clone()).unwrap_or_default();
    if settings.purge_consent != Some(true) {
        return 0;
    }
    let policy = load_policy(state);
    let Some(letter) = policy.drive_for(Role::Disposable) else {
        return 0;
    };
    let freed = smd_win::quarantine::purge(
        &smd_win::quarantine::root_for(letter),
        settings.quarantine_days,
        now_ms(),
    );
    if freed > 0 {
        let mut e = Entry::new(state.next_id(), activity::Kind::QuarantinePurge);
        e.bytes = freed;
        activity::record(state, e);
        log::info!("quarantine purge freed {freed} bytes");
    }
    freed
}
