//! Performance: startup items (turn off/on, with undo), live programs, background tasks and services, boot.

use serde::{Deserialize, Serialize};
use smd_win::background::{BootInfo, ScheduledTask, Service};
use smd_win::processes::{Monitor, Snapshot};
use smd_win::startup::StartupItem;
use tauri::{AppHandle, Manager};

use crate::activity::{self, Entry, StartupUndo, Undo, UndoData};
use crate::error::{AppError, AppResult};
use crate::state::{AppState, lock};
use crate::store::now_ms;

const TOP_PROGRAMS: usize = 12;

async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> AppResult<T> + Send + 'static,
) -> AppResult<T> {
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
}

#[tauri::command]
pub async fn startup_list() -> AppResult<Vec<StartupItem>> {
    blocking(|| Ok(smd_win::startup::list())).await
}

/// Turns a startup item off or on (your user scope only) and records an undo.
#[tauri::command]
pub async fn startup_set(app: AppHandle, id: String, enabled: bool) -> AppResult<Vec<StartupItem>> {
    blocking(move || {
        let state = app.state::<AppState>();
        let previous =
            smd_win::startup::set_enabled(&id, enabled, now_ms()).map_err(|e| match e.kind() {
                std::io::ErrorKind::PermissionDenied => AppError::AccessDenied(e.to_string()),
                std::io::ErrorKind::NotFound => AppError::NotFound(e.to_string()),
                _ => AppError::Io(e),
            })?;
        let name = id
            .split_once(':')
            .map(|(_, n)| n.to_string())
            .unwrap_or_else(|| id.clone());
        let entry_id = state.next_id();
        let undo = UndoData {
            startup: Some(StartupUndo {
                id: id.clone(),
                was_enabled: previous,
            }),
            ..Default::default()
        };
        let mut e = Entry::new(entry_id, activity::Kind::StartupChange);
        e.target = Some(name);
        e.enabled = Some(enabled);
        e.items = 1;
        e.undo = if previous != enabled && activity::save_undo(&state, entry_id, &undo) {
            Undo::Available
        } else {
            Undo::None
        };
        activity::record(&state, e);
        log::info!(
            "startup item {id} turned {}",
            if enabled { "on" } else { "off" }
        );
        Ok(smd_win::startup::list())
    })
    .await
}

/// Top programs by CPU and memory. Call it every couple of seconds while the page is open.
#[tauri::command]
pub async fn processes_snapshot(app: AppHandle) -> AppResult<Snapshot> {
    blocking(move || {
        let state = app.state::<AppState>();
        let mut slot = lock(&state.monitor)?;
        if slot.is_none() {
            *slot = Some(Monitor::new()?);
        }
        Ok(slot.as_mut().expect("just set").snapshot(TOP_PROGRAMS))
    })
    .await
}

/// Frees the process monitor when the page closes (keeps idle memory low).
#[tauri::command]
pub fn processes_stop(state: tauri::State<'_, AppState>) -> AppResult<()> {
    *lock(&state.monitor)? = None;
    Ok(())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Background {
    pub tasks: Option<Vec<ScheduledTask>>,
    pub services: Option<Vec<Service>>,
    pub boot: Option<BootInfo>,
}

#[tauri::command]
pub async fn background_list() -> AppResult<Background> {
    blocking(|| {
        let (tasks, services, boot) = std::thread::scope(|s| {
            let t = s.spawn(smd_win::background::scheduled_tasks);
            let v = s.spawn(smd_win::background::third_party_services);
            let b = smd_win::background::boot_info();
            (
                t.join().ok().and_then(|r| r.ok()),
                v.join().ok().and_then(|r| r.ok()),
                b.ok(),
            )
        });
        Ok(Background {
            tasks,
            services,
            boot,
        })
    })
    .await
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SystemTool {
    TaskScheduler,
    Services,
    WindowsUpdate,
    WindowsSecurity,
    PrivacySettings,
}

/// Opens a Windows tool. Only these, never an arbitrary path.
#[tauri::command]
pub fn open_system_tool(tool: SystemTool) -> AppResult<()> {
    let windir = std::env::var("WINDIR").unwrap_or_else(|_| r"C:\Windows".into());
    let file = match tool {
        SystemTool::TaskScheduler => "taskschd.msc",
        SystemTool::Services => "services.msc",
        SystemTool::WindowsUpdate | SystemTool::WindowsSecurity | SystemTool::PrivacySettings => {
            let url = match tool {
                SystemTool::WindowsUpdate => "ms-settings:windowsupdate",
                SystemTool::WindowsSecurity => "windowsdefender:",
                _ => "ms-settings:privacy",
            };
            return tauri_plugin_opener::open_url(url, None::<&str>)
                .map_err(|e| AppError::Internal(e.to_string()));
        }
    };
    let path = std::path::Path::new(&windir).join("System32").join(file);
    tauri_plugin_opener::open_path(path, None::<&str>)
        .map_err(|e| AppError::Internal(e.to_string()))
}
