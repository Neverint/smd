//! Shared backend state.

use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use smd_core::space::Arena;
use smd_win::apps::{InstalledApp, Leftover};
use smd_win::cleanup::Category;
use smd_win::clutter::{ClutterResult, FolderItem};
use smd_win::hardware::SensorHelper;
use smd_win::processes::Monitor;

use crate::error::{AppError, AppResult};
use crate::settings::Settings;
use crate::window_effect::WindowEffect;

/// The last Cleanup scan. A run only acts on items from this scan.
pub struct CleanupScan {
    pub id: u64,
    pub categories: Vec<Category>,
    /// The admin categories were scanned again by the elevated helper (and can be cleaned).
    pub admin_scanned: bool,
}

pub struct SpaceScan {
    pub id: u64,
    pub arena: Arena,
}

/// The last My Clutter scan or Downloads listing. Removal only acts on files listed here.
pub struct ClutterScan {
    pub id: u64,
    pub result: ClutterResult,
}

pub struct DownloadsList {
    pub id: u64,
    pub root: PathBuf,
    pub items: Vec<FolderItem>,
}

/// Leftovers found after an uninstall.
pub struct LeftoverScan {
    pub id: u64,
    pub app_name: String,
    pub items: Vec<Leftover>,
}

pub struct AppState {
    pub window_effect: WindowEffect,
    /// `%LOCALAPPDATA%\io.github.neverint.smd`
    pub data_dir: PathBuf,
    pub settings: Mutex<Settings>,
    pub cleanup: Mutex<Option<CleanupScan>>,
    pub space: Mutex<Option<SpaceScan>>,
    pub space_cancel: AtomicBool,
    /// Set while a Smart Care or Cleanup scan, a cleanup or a restore runs, so two never overlap.
    pub busy: AtomicBool,
    /// Set while Space Lens walks a drive (read-only, so it may run alongside the others).
    pub space_busy: AtomicBool,
    pub activity_lock: Mutex<()>,
    /// Last installed-apps list: uninstall and leftovers only act on apps listed here.
    pub apps: Mutex<Vec<InstalledApp>>,
    pub leftovers: Mutex<Option<LeftoverScan>>,
    pub clutter: Mutex<Option<ClutterScan>>,
    pub downloads: Mutex<Option<DownloadsList>>,
    pub clutter_cancel: AtomicBool,
    pub clutter_busy: AtomicBool,
    /// Uninstalls and updates run their own installers one at a time.
    pub apps_busy: AtomicBool,
    pub monitor: Mutex<Option<Monitor>>,
    /// The LibreHardwareMonitor helper, while the Hardware Health page is open.
    pub sensors: Mutex<Option<SensorHelper>>,
    /// A Defender scan in progress (killed to cancel).
    pub defender_scan: Mutex<Option<std::process::Child>>,
    /// Drive Organizer: last audit, the plan waiting for a go, and the cancel switch of a running move.
    pub organizer_audit: Mutex<Option<crate::organizer::AuditState>>,
    pub organizer_plan: Mutex<Option<crate::organizer::PlanState>>,
    pub organizer_cancel: AtomicBool,
    /// The elevated helper session (UAC once per batch; exits after 10 idle minutes).
    pub helper: Mutex<Option<smd_win::elevate::Client>>,
    /// Last reading of the sensor helper running with admin rights.
    pub admin_sensors: Mutex<Option<smd_win::hardware::SensorSnapshot>>,
    pub toolbox: Mutex<crate::toolbox::ToolboxCache>,
    /// The tray popover's live numbers.
    pub sampler: Mutex<Option<smd_win::perf::Sampler>>,
    pub popover_hidden: Mutex<Option<std::time::Instant>>,
    /// The window was hidden while a task runs: a thread waits to notify when it ends.
    pub watching_task: AtomicBool,
    next_id: AtomicU64,
}

impl AppState {
    pub fn new(window_effect: WindowEffect, data_dir: PathBuf) -> Self {
        AppState {
            window_effect,
            data_dir,
            settings: Mutex::new(Settings::default()),
            cleanup: Mutex::new(None),
            space: Mutex::new(None),
            space_cancel: AtomicBool::new(false),
            busy: AtomicBool::new(false),
            space_busy: AtomicBool::new(false),
            activity_lock: Mutex::new(()),
            apps: Mutex::new(Vec::new()),
            leftovers: Mutex::new(None),
            clutter: Mutex::new(None),
            downloads: Mutex::new(None),
            clutter_cancel: AtomicBool::new(false),
            clutter_busy: AtomicBool::new(false),
            apps_busy: AtomicBool::new(false),
            monitor: Mutex::new(None),
            sensors: Mutex::new(None),
            defender_scan: Mutex::new(None),
            organizer_audit: Mutex::new(None),
            organizer_plan: Mutex::new(None),
            organizer_cancel: AtomicBool::new(false),
            helper: Mutex::new(None),
            admin_sensors: Mutex::new(None),
            toolbox: Mutex::new(Default::default()),
            sampler: Mutex::new(None),
            popover_hidden: Mutex::new(None),
            watching_task: AtomicBool::new(false),
            next_id: AtomicU64::new(crate::store::now_ms() as u64),
        }
    }

    /// Unique, increasing ids for scans and activity entries.
    pub fn next_id(&self) -> u64 {
        self.next_id.fetch_add(1, Ordering::Relaxed)
    }

    /// Marks the backend busy until the guard is dropped.
    pub fn begin(&self, what: &str) -> AppResult<BusyGuard<'_>> {
        guard(&self.busy, what)
    }

    pub fn begin_space(&self) -> AppResult<BusyGuard<'_>> {
        guard(&self.space_busy, "a Space Lens scan")
    }

    pub fn begin_clutter(&self) -> AppResult<BusyGuard<'_>> {
        guard(&self.clutter_busy, "a My Clutter scan")
    }

    pub fn begin_apps(&self, what: &str) -> AppResult<BusyGuard<'_>> {
        guard(&self.apps_busy, what)
    }

    /// Any scan, cleanup, move, install or Defender scan is running.
    pub fn any_busy(&self) -> bool {
        [
            &self.busy,
            &self.space_busy,
            &self.clutter_busy,
            &self.apps_busy,
        ]
        .iter()
        .any(|f| f.load(Ordering::Acquire))
            || self.defender_scan.lock().is_ok_and(|d| d.is_some())
    }
}

fn guard<'a>(flag: &'a AtomicBool, what: &str) -> AppResult<BusyGuard<'a>> {
    if flag.swap(true, Ordering::AcqRel) {
        return Err(AppError::Busy(format!(
            "cannot start {what} while another task runs"
        )));
    }
    Ok(BusyGuard(flag))
}

pub struct BusyGuard<'a>(&'a AtomicBool);

impl Drop for BusyGuard<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

pub fn lock<T>(m: &Mutex<T>) -> AppResult<std::sync::MutexGuard<'_, T>> {
    m.lock()
        .map_err(|_| AppError::Internal("state lock poisoned".into()))
}
