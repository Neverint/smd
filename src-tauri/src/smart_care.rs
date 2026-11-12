//! Smart Care: one read-only pass over every check built so far, then a health score.

use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use serde::Serialize;
use smd_core::health::{Assessment, Facts, PendingUpdates, ProtectionFacts, assess};
use smd_win::disks::PhysicalDisk;
use smd_win::protection::DefenderStatus;
use smd_win::system::{DriveInfo, MemoryInfo};
use smd_win::winget::WindowsUpdates;
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager};

use crate::activity::{self, Entry};
use crate::cleanup::{CleanupScanResult, ScanProgress, rule_count, scan_all};
use crate::error::{AppError, AppResult};
use crate::state::AppState;
use crate::store::now_ms;

/// Memory is sampled a few times during the scan; "sustained" use is the lowest sample.
const MEMORY_SAMPLES: u32 = 3;
const MEMORY_INTERVAL: Duration = Duration::from_millis(900);
/// Update checks are skipped (not guessed) when they take longer than this.
const UPDATES_TIMEOUT: Duration = Duration::from_secs(25);

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SmartCareResult {
    pub assessment: Assessment,
    pub system_drive: Option<DriveInfo>,
    pub memory: Option<MemoryInfo>,
    pub memory_min_load: Option<f64>,
    pub uptime_seconds: u64,
    pub disks: Option<Vec<PhysicalDisk>>,
    pub has_battery: bool,
    pub cleanup: CleanupScanResult,
    /// Third-party startup items that are on.
    pub startup_enabled: Option<usize>,
    /// Apps with an update (only when the WinGet module is set up).
    pub app_updates: Option<u32>,
    pub windows_updates: Option<WindowsUpdates>,
    pub defender: Option<DefenderStatus>,
    /// Threats Defender still lists as needing attention.
    pub active_threats: Option<u32>,
    /// Lowest battery health in percent (laptops only).
    pub battery_health: Option<f64>,
    pub duration_ms: u64,
    pub activity_id: u64,
}

fn sample_memory() -> (Option<MemoryInfo>, Option<f64>) {
    let mut last = None;
    let mut min_load: Option<f64> = None;
    for i in 0..MEMORY_SAMPLES {
        if i > 0 {
            std::thread::sleep(MEMORY_INTERVAL);
        }
        if let Ok(m) = smd_win::system::memory() {
            let load = f64::from(m.load_percent);
            min_load = Some(min_load.map_or(load, |v| v.min(load)));
            last = Some(m);
        }
    }
    (last, min_load)
}

pub fn scan(state: &AppState, on_progress: &Channel<ScanProgress>) -> SmartCareResult {
    let started = now_ms();
    let total = rule_count() + 6;
    let done = AtomicU32::new(0);
    let report = |step: &str| {
        let n = done.fetch_add(1, Ordering::Relaxed) + 1;
        let _ = on_progress.send(ScanProgress {
            step: step.to_string(),
            done: n,
            total,
        });
    };

    let (
        disks,
        (memory, min_load),
        cleanup,
        startup_enabled,
        (app_updates, windows_updates),
        (defender, active_threats, battery_health),
    ) = std::thread::scope(|s| {
        let protection = s.spawn(|| {
            let d = smd_win::protection::defender_status().ok();
            let t = smd_win::protection::threats()
                .ok()
                .map(|l| l.iter().filter(|t| t.active).count() as u32);
            let temp = state.data_dir.join("tmp");
            let b = smd_win::hardware::batteries(&temp)
                .ok()
                .and_then(|l| l.iter().map(|b| b.health_percent()).reduce(f64::min));
            report("protection");
            (d, t, b)
        });
        let startup = s.spawn(|| {
            let n = smd_win::startup::list()
                .iter()
                .filter(|i| i.enabled && !i.microsoft)
                .count();
            report("startup");
            n
        });
        let updates = s.spawn(|| {
            let apps = s.spawn(|| match smd_win::winget::module_installed() {
                Ok(true) => smd_win::winget::list_updates(UPDATES_TIMEOUT)
                    .map(|l| l.len() as u32)
                    .ok(),
                _ => None,
            });
            let wu = smd_win::winget::windows_updates(UPDATES_TIMEOUT)
                .map_err(|e| log::info!("Windows Update check skipped: {e}"))
                .ok();
            let a = apps.join().ok().flatten();
            report("updates");
            (a, wu)
        });
        let disks = s.spawn(|| {
            let d = smd_win::disks::physical_disks();
            if let Err(e) = &d {
                log::warn!("disk health check failed: {e}");
            }
            report("disks");
            d.ok()
        });
        let mem = s.spawn(|| {
            let m = sample_memory();
            report("memory");
            m
        });
        let cleanup = scan_all(state, &report);
        (
            disks.join().ok().flatten(),
            mem.join().unwrap_or((None, None)),
            cleanup,
            startup.join().ok(),
            updates.join().unwrap_or((None, None)),
            protection.join().unwrap_or((None, None, None)),
        )
    });

    let system_letter = smd_win::system::system_drive().to_uppercase();
    let system_drive = smd_win::system::drives()
        .into_iter()
        .find(|d| d.letter == system_letter);
    report("drive");
    let uptime_seconds = smd_win::system::uptime_seconds();

    let facts = Facts {
        system_drive: system_letter,
        system_free_percent: system_drive.as_ref().map(DriveInfo::free_percent),
        uptime_days: Some(uptime_seconds as f64 / 86_400.0),
        memory_min_load_percent: min_load,
        memory_commit_percent: memory.map(|m| m.commit_percent),
        disks: disks
            .as_ref()
            .map(|d| d.iter().map(|x| (x.name.clone(), x.health)).collect()),
        reclaimable_bytes: cleanup.reclaimable_bytes,
        updates: PendingUpdates {
            apps: app_updates,
            windows: windows_updates.as_ref().map(|w| w.total),
            security: windows_updates.as_ref().map(|w| w.security),
        },
        startup_enabled_third_party: startup_enabled,
        protection: defender.as_ref().map(|d| ProtectionFacts {
            real_time: d.real_time,
            other_antivirus: d.running_mode.eq_ignore_ascii_case("Passive")
                || d.running_mode.eq_ignore_ascii_case("EDR Block Mode"),
            signature_age_days: d.signature_age_days,
            active_threats: active_threats.unwrap_or(0),
        }),
        battery_health_percent: battery_health,
    };
    let assessment = assess(&facts);

    let id = state.next_id();
    let mut e = Entry::new(id, activity::Kind::SmartCareScan);
    e.score = Some(assessment.score);
    e.bytes = cleanup.reclaimable_bytes;
    activity::record(state, e);
    log::info!(
        "smart care {id}: score {} ({:?})",
        assessment.score,
        assessment.verdict
    );

    SmartCareResult {
        assessment,
        system_drive,
        memory,
        memory_min_load: min_load,
        uptime_seconds,
        disks,
        has_battery: smd_win::system::has_battery(),
        cleanup,
        startup_enabled,
        app_updates,
        windows_updates,
        defender,
        active_threats,
        battery_health,
        duration_ms: (now_ms() - started).max(0) as u64,
        activity_id: id,
    }
}

#[tauri::command]
pub async fn smart_care_scan(
    app: AppHandle,
    on_progress: Channel<ScanProgress>,
) -> AppResult<SmartCareResult> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let _busy = state.begin("Smart Care")?;
        Ok(scan(&state, &on_progress))
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))?
}
