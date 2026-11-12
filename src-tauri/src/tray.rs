//! Tray icon, its mini-monitor popover, background Smart Care (tray, scheduled task) and
//! "keep running in the tray" while a task finishes.

use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use smd_win::perf::Sample;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, Rect, WebviewUrl, WebviewWindowBuilder, WindowEvent};

use crate::activity::{self, Kind};
use crate::error::{AppError, AppResult};
use crate::notify::{self, Text};
use crate::state::{AppState, lock};

const POPOVER: &str = "tray";
const POPOVER_W: f64 = 340.0;
const POPOVER_H: f64 = 470.0;

pub fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
    if let Some(p) = app.get_webview_window(POPOVER) {
        let _ = p.hide();
    }
}

pub fn setup(app: &AppHandle) -> tauri::Result<()> {
    let state = app.state::<AppState>();
    let open = MenuItem::with_id(
        app,
        "open",
        notify::text(&state, Text::TrayOpen),
        true,
        None::<&str>,
    )?;
    let care = MenuItem::with_id(
        app,
        "smartCare",
        notify::text(&state, Text::TraySmartCare),
        true,
        None::<&str>,
    )?;
    let quit = MenuItem::with_id(
        app,
        "quit",
        notify::text(&state, Text::TrayQuit),
        true,
        None::<&str>,
    )?;
    let menu = Menu::with_items(
        app,
        &[&open, &care, &PredefinedMenuItem::separator(app)?, &quit],
    )?;
    let mut builder = TrayIconBuilder::with_id("main")
        .tooltip("Scrub My Disks")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, e| match e.id.as_ref() {
            "open" => show_main(app),
            "smartCare" => run_smart_care(app, false),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, e| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                rect,
                ..
            } = e
            {
                toggle_popover(tray.app_handle(), rect);
            }
        });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;
    Ok(())
}

fn popover(app: &AppHandle) -> tauri::Result<tauri::WebviewWindow> {
    if let Some(w) = app.get_webview_window(POPOVER) {
        return Ok(w);
    }
    let w = WebviewWindowBuilder::new(app, POPOVER, WebviewUrl::App("index.html#/tray".into()))
        .title("Scrub My Disks")
        .inner_size(POPOVER_W, POPOVER_H)
        .decorations(false)
        .transparent(true)
        .resizable(false)
        .skip_taskbar(true)
        .always_on_top(true)
        .shadow(true)
        .visible(false)
        .build()?;
    let _ = crate::window_effect::apply(&w);
    let handle = app.clone();
    w.on_window_event(move |e| {
        if let WindowEvent::Focused(false) = e
            && let Some(w) = handle.get_webview_window(POPOVER)
        {
            let _ = w.hide();
            if let Ok(mut t) = handle.state::<AppState>().popover_hidden.lock() {
                *t = Some(Instant::now());
            }
        }
    });
    Ok(w)
}

fn toggle_popover(app: &AppHandle, rect: Rect) {
    let Ok(w) = popover(app) else { return };
    if w.is_visible().unwrap_or(false) {
        let _ = w.hide();
        return;
    }
    // A click on the icon first takes the focus away (which hides it): don't reopen at once.
    if let Ok(t) = app.state::<AppState>().popover_hidden.lock()
        && t.is_some_and(|t| t.elapsed() < Duration::from_millis(300))
    {
        return;
    }
    let scale = w.scale_factor().unwrap_or(1.0);
    let pos = rect.position.to_physical::<f64>(scale);
    let size = rect.size.to_physical::<f64>(scale);
    let (ww, wh) = (POPOVER_W * scale, POPOVER_H * scale);
    let monitor = w
        .monitor_from_point(pos.x, pos.y)
        .ok()
        .flatten()
        .or_else(|| w.primary_monitor().ok().flatten());
    let (mx, my, mw, mh) = monitor
        .map(|m| {
            let a = m.work_area();
            (
                a.position.x as f64,
                a.position.y as f64,
                a.size.width as f64,
                a.size.height as f64,
            )
        })
        .unwrap_or((0.0, 0.0, 1920.0 * scale, 1080.0 * scale));
    let mut x = pos.x + size.width / 2.0 - ww / 2.0;
    let mut y = if pos.y > my + mh / 2.0 {
        pos.y - wh - 8.0 * scale
    } else {
        pos.y + size.height + 8.0 * scale
    };
    x = x.clamp(mx + 8.0, (mx + mw - ww - 8.0).max(mx));
    y = y.clamp(my + 8.0, (my + mh - wh - 8.0).max(my));
    let _ = w.set_position(tauri::PhysicalPosition::new(x, y));
    let _ = w.show();
    let _ = w.set_focus();
}

/// Runs Smart Care in the background and shows the score in a notification.
/// `exit_after`: the app was started only for this (scheduled task) and closes afterwards.
pub fn run_smart_care(app: &AppHandle, exit_after: bool) {
    let app = app.clone();
    std::thread::spawn(move || {
        let state = app.state::<AppState>();
        let result = state
            .begin("Smart Care")
            .map(|_busy| crate::smart_care::scan(&state, &|_| {}));
        match result {
            Ok(r) => {
                let body = notify::text(&state, Text::SmartCareDone)
                    .replace("{score}", &r.assessment.score.to_string());
                notify::show(&app, "Scrub My Disks", &body);
            }
            Err(e) => {
                log::warn!("background Smart Care: {e}");
                notify::show(
                    &app,
                    "Scrub My Disks",
                    notify::text(&state, Text::SmartCareFailed),
                );
            }
        }
        let window_open = app
            .get_webview_window("main")
            .is_some_and(|w| w.is_visible().unwrap_or(false));
        if exit_after && !window_open {
            // Let Windows take the notification before the process ends.
            std::thread::sleep(Duration::from_secs(3));
            app.exit(0);
        }
    });
}

/// Main window close: keep running in the tray while a task runs (or if the user asked),
/// and say when the task is done. Otherwise quit.
pub fn on_main_close(app: &AppHandle, api: &tauri::CloseRequestApi) {
    let state = app.state::<AppState>();
    let busy = state.any_busy();
    let keep = lock(&state.settings).is_ok_and(|s| s.close_to_tray);
    if !busy && !keep {
        app.exit(0);
        return;
    }
    api.prevent_close();
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.hide();
    }
    if busy && !state.watching_task.swap(true, Ordering::AcqRel) {
        let app = app.clone();
        std::thread::spawn(move || {
            let state = app.state::<AppState>();
            while state.any_busy() {
                std::thread::sleep(Duration::from_secs(1));
            }
            state.watching_task.store(false, Ordering::Release);
            notify::show(&app, "Scrub My Disks", notify::text(&state, Text::TaskDone));
        });
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrayInfo {
    pub system_drive: String,
    pub system_free: u64,
    pub system_total: u64,
    pub recycle_bin_bytes: u64,
    pub last_score: Option<u8>,
    pub last_score_ms: Option<i64>,
    /// From the sensor helper, when Hardware Health (or its admin mode) is running.
    pub cpu_temp_c: Option<f64>,
    pub busy: bool,
}

fn cpu_temp(state: &AppState) -> Option<f64> {
    let local = lock(&state.sensors)
        .ok()
        .and_then(|s| s.as_ref().and_then(|h| h.latest()));
    let snap = local.or_else(|| lock(&state.admin_sensors).ok().and_then(|s| s.clone()))?;
    snap.hardware
        .iter()
        .filter(|h| h.kind.eq_ignore_ascii_case("cpu"))
        .filter_map(smd_core::hardware::max_temperature)
        .fold(None, |m: Option<f64>, v| Some(m.map_or(v, |m| m.max(v))))
}

#[tauri::command]
pub async fn tray_info(app: AppHandle) -> AppResult<TrayInfo> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let sys = smd_win::system::system_drive();
        let drive = smd_win::system::drives()
            .into_iter()
            .find(|d| d.letter.eq_ignore_ascii_case(&sys));
        let last = activity::latest(&state, Kind::SmartCareScan);
        Ok(TrayInfo {
            system_free: drive.as_ref().map_or(0, |d| d.free_bytes),
            system_total: drive.as_ref().map_or(0, |d| d.total_bytes),
            system_drive: sys,
            recycle_bin_bytes: smd_win::system::recycle_bin_size().map_or(0, |(b, _)| b),
            last_score: last.as_ref().and_then(|e| e.score),
            last_score_ms: last.map(|e| e.time_ms),
            cpu_temp_c: cpu_temp(&state),
            busy: state.any_busy(),
        })
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))?
}

/// CPU, memory, disk and network since the previous call (the popover calls it every second).
#[tauri::command]
pub fn tray_sample(state: tauri::State<'_, AppState>) -> AppResult<Sample> {
    let mut slot = lock(&state.sampler)?;
    if slot.is_none() {
        *slot = Some(smd_win::perf::Sampler::new()?);
    }
    Ok(slot.as_mut().expect("just made").sample())
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TrayAction {
    OpenApp,
    SmartCare,
    EmptyRecycleBin,
}

#[tauri::command]
pub async fn tray_action(app: AppHandle, action: TrayAction) -> AppResult<()> {
    match action {
        TrayAction::OpenApp => show_main(&app),
        TrayAction::SmartCare => run_smart_care(&app, false),
        TrayAction::EmptyRecycleBin => {
            tauri::async_runtime::spawn_blocking(move || {
                let state = app.state::<AppState>();
                let _busy = state.begin("emptying the Recycle Bin")?;
                let (bytes, _) = smd_win::system::recycle_bin_size().unwrap_or((0, 0));
                smd_win::system::empty_recycle_bin()?;
                let mut e = activity::Entry::new(state.next_id(), Kind::EmptyRecycleBin);
                e.bytes = bytes;
                activity::record(&state, e);
                Ok::<_, AppError>(())
            })
            .await
            .map_err(|e| AppError::Internal(e.to_string()))??;
        }
    }
    Ok(())
}

/// The popover asks to close itself (Escape key).
#[tauri::command]
pub fn tray_hide(app: AppHandle) {
    if let Some(w) = app.get_webview_window(POPOVER) {
        let _ = w.hide();
    }
}
