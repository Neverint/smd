//! Ongoing guard: while the app runs, watch the game library folders and your profile folder.
//! After a change settles, run the Drive Organizer audit and notify about new problems
//! (for example "47 GB game installed on G:, your layout puts it on X:").

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};
use std::time::Duration;

use smd_core::organizer::Severity;
use tauri::{AppHandle, Manager};

use crate::notify;
use crate::state::{AppState, lock};
use crate::store::{self, now_ms};

const SEEN_FILE: &str = "guard-seen.json";
/// Wait this long after the last change (an install takes a while), then audit.
const SETTLE_MS: i64 = 10 * 60 * 1000;

fn enabled(state: &AppState) -> bool {
    lock(&state.settings).is_ok_and(|s| s.guard)
}

/// Problems found now, by finding id (path-based, so stable between audits).
fn problems(state: &AppState) -> Option<Vec<smd_core::organizer::Finding>> {
    let view = crate::organizer::run_audit(state).ok()?;
    Some(
        view.findings
            .into_iter()
            .filter(|f| f.severity == Severity::Problem && f.target_drive.is_some())
            .collect(),
    )
}

fn check(app: &AppHandle, first: bool) {
    let state = app.state::<AppState>();
    let Some(found) = problems(&state) else {
        return;
    };
    let path = state.data_dir.join(SEEN_FILE);
    let seen: Option<HashSet<String>> = store::read_json(&path).ok().flatten();
    let mut seen_now = seen.clone().unwrap_or_default();
    // First run ever: remember what is already there, notify nothing.
    if seen.is_some() && !first {
        for f in found.iter().filter(|f| !seen_now.contains(&f.id)) {
            let size = smd_core::format::format_bytes(f.bytes);
            let body = notify::guard_body(
                &state,
                &f.name,
                &size,
                &f.drive,
                f.target_drive.as_deref().unwrap_or("?"),
            );
            notify::show(app, notify::text(&state, notify::Text::GuardTitle), &body);
        }
    }
    seen_now.extend(found.into_iter().map(|f| f.id));
    if let Err(e) = store::write_json(&path, &seen_now) {
        log::warn!("guard: could not save what it has seen: {e}");
    }
}

/// Starts the watchers and the settle-then-audit loop (background threads).
pub fn start(app: &AppHandle) {
    let app = app.clone();
    std::thread::spawn(move || {
        // Leave the start-up to the window first.
        std::thread::sleep(Duration::from_secs(90));
        let state = app.state::<AppState>();
        if enabled(&state) {
            check(&app, !state.data_dir.join(SEEN_FILE).exists());
        }
        let drives = smd_win::system::drives();
        let mut dirs: HashSet<PathBuf> = smd_win::organizer::all_games(&drives)
            .iter()
            .filter_map(|g| PathBuf::from(&g.path).parent().map(PathBuf::from))
            .collect();
        if let Ok(home) = std::env::var("USERPROFILE") {
            dirs.insert(PathBuf::from(home));
        }
        let changed = Arc::new(AtomicI64::new(0));
        for d in &dirs {
            let c = Arc::clone(&changed);
            if let Err(e) =
                smd_win::watch::watch_folder(d, move || c.store(now_ms(), Ordering::Relaxed))
            {
                log::warn!("guard: cannot watch {}: {e}", d.display());
            }
        }
        log::info!("guard: watching {} folders", dirs.len());
        loop {
            std::thread::sleep(Duration::from_secs(60));
            let at = changed.load(Ordering::Relaxed);
            if at != 0 && now_ms() - at > SETTLE_MS {
                changed.store(0, Ordering::Relaxed);
                if enabled(&state) && !state.any_busy() {
                    check(&app, false);
                }
            }
        }
    });
}
