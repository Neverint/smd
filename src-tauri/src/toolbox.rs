//! Toolbox: trusted free tools, installed, updated and opened through winget.

use std::collections::HashMap;

use serde::Serialize;
use smd_core::toolbox::{Tool, builtin_tools};
use smd_win::toolbox::{StartApp, WingetResult};
use tauri::{AppHandle, Manager};

use crate::activity::{self, Entry};
use crate::error::{AppError, AppResult};
use crate::state::{AppState, lock};
use crate::store::now_ms;

/// What winget reported as installed, and the Start menu apps (both slow to read).
#[derive(Default)]
pub struct ToolboxCache {
    pub checked_ms: i64,
    pub installed: HashMap<String, Option<String>>,
    pub start_apps: Vec<StartApp>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolView {
    #[serde(flatten)]
    pub tool: Tool,
    pub installed: bool,
    pub version: Option<String>,
    pub can_open: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolboxView {
    pub tools: Vec<ToolView>,
    /// When winget was last asked (0: winget is not available).
    pub checked_ms: i64,
    pub winget_error: Option<String>,
}

fn can_open(tool: &Tool, apps: &[StartApp]) -> bool {
    tool.open.start_app.as_deref().is_some_and(|n| {
        apps.iter()
            .any(|a| smd_win::toolbox::start_app_matches(&a.name, n))
    }) || smd_win::toolbox::portable_exe(tool).is_some()
}

fn view(cache: &ToolboxCache, winget_error: Option<String>) -> ToolboxView {
    let tools = builtin_tools()
        .into_iter()
        .map(|t| {
            let entry = cache.installed.get(&t.winget_id.to_lowercase());
            ToolView {
                installed: entry.is_some(),
                version: entry.cloned().flatten(),
                can_open: entry.is_some() && can_open(&t, &cache.start_apps),
                tool: t,
            }
        })
        .collect();
    ToolboxView {
        tools,
        checked_ms: cache.checked_ms,
        winget_error,
    }
}

fn refresh(state: &AppState) -> Option<String> {
    // winget is a packaged app: its writes under AppData are redirected, so use the temp folder.
    let temp = std::env::temp_dir();
    let (installed, apps) = std::thread::scope(|s| {
        let apps = s.spawn(smd_win::toolbox::start_apps);
        (
            smd_win::toolbox::installed(&temp),
            apps.join().ok().and_then(|r| r.ok()).unwrap_or_default(),
        )
    });
    let Ok(mut cache) = lock(&state.toolbox) else {
        return Some("state lock".into());
    };
    cache.start_apps = apps;
    match installed {
        Ok(m) => {
            cache.installed = m;
            cache.checked_ms = now_ms();
            None
        }
        Err(e) => {
            log::warn!("winget export failed: {e}");
            Some(e.to_string())
        }
    }
}

async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> AppResult<T> + Send + 'static,
) -> AppResult<T> {
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
}

/// The catalog with what is installed. `refresh` asks winget again (about 20 s).
#[tauri::command]
pub async fn toolbox_list(app: AppHandle, refresh_now: bool) -> AppResult<ToolboxView> {
    blocking(move || {
        let state = app.state::<AppState>();
        let stale = lock(&state.toolbox)?.checked_ms == 0;
        let err = if refresh_now || stale {
            refresh(&state)
        } else {
            None
        };
        Ok(view(&*lock(&state.toolbox)?, err))
    })
    .await
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolActionResult {
    pub result: WingetResult,
    pub view: ToolboxView,
}

fn find(id: &str) -> AppResult<Tool> {
    builtin_tools()
        .into_iter()
        .find(|t| t.id == id)
        .ok_or_else(|| AppError::NotFound(format!("tool {id}")))
}

/// Installs (or updates) a tool with winget. Needs the internet; the installer may ask for admin.
#[tauri::command]
pub async fn toolbox_install(
    app: AppHandle,
    id: String,
    update: bool,
) -> AppResult<ToolActionResult> {
    blocking(move || {
        let state = app.state::<AppState>();
        let _busy = state.begin_apps("a Toolbox install")?;
        let tool = find(&id)?;
        log::info!(
            "toolbox: {} {}",
            if update { "updating" } else { "installing" },
            tool.winget_id
        );
        let result = if update {
            smd_win::toolbox::upgrade(&tool.winget_id)?
        } else {
            smd_win::toolbox::install(&tool.winget_id)?
        };
        let mut e = Entry::new(
            state.next_id(),
            if update {
                activity::Kind::ToolUpdate
            } else {
                activity::Kind::ToolInstall
            },
        );
        e.target = Some(tool.name.clone());
        e.items = u64::from(result.ok);
        e.failed = u64::from(!result.ok);
        activity::record(&state, e);
        let err = if result.ok { refresh(&state) } else { None };
        Ok(ToolActionResult {
            result,
            view: view(&*lock(&state.toolbox)?, err),
        })
    })
    .await
}

#[tauri::command]
pub async fn toolbox_open(app: AppHandle, id: String) -> AppResult<()> {
    blocking(move || {
        let state = app.state::<AppState>();
        let tool = find(&id)?;
        let apps = lock(&state.toolbox)?.start_apps.clone();
        smd_win::toolbox::open(&tool, &apps).map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => AppError::NotFound(e.to_string()),
            _ => AppError::Io(e),
        })
    })
    .await
}
