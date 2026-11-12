//! Scheduled Smart Care settings: a Task Scheduler task for your account (scan only).

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::error::{AppError, AppResult};
use crate::state::{AppState, lock};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum Frequency {
    #[default]
    Off,
    Daily,
    Weekly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Schedule {
    pub frequency: Frequency,
    /// 0 = Monday ... 6 = Sunday (weekly only).
    pub day: u8,
    /// 0-23, local time.
    pub hour: u8,
}

impl Default for Schedule {
    fn default() -> Self {
        Schedule {
            frequency: Frequency::Off,
            day: 6,
            hour: 10,
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScheduleView {
    pub schedule: Schedule,
    /// The task exists in Task Scheduler.
    pub registered: bool,
}

async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> AppResult<T> + Send + 'static,
) -> AppResult<T> {
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
}

#[tauri::command]
pub async fn schedule_get(app: AppHandle) -> AppResult<ScheduleView> {
    blocking(move || {
        let state = app.state::<AppState>();
        let schedule = lock(&state.settings)?.schedule;
        Ok(ScheduleView {
            schedule,
            registered: smd_win::schedule::exists().unwrap_or(false),
        })
    })
    .await
}

#[tauri::command]
pub async fn schedule_set(app: AppHandle, schedule: Schedule) -> AppResult<ScheduleView> {
    blocking(move || {
        let state = app.state::<AppState>();
        if schedule.day > 6 || schedule.hour > 23 {
            return Err(AppError::Internal("bad schedule".into()));
        }
        match schedule.frequency {
            Frequency::Off => smd_win::schedule::unregister()?,
            f => smd_win::schedule::register(
                &std::env::current_exe()?,
                f == Frequency::Daily,
                schedule.day,
                schedule.hour,
            )?,
        }
        let settings = {
            let mut s = lock(&state.settings)?;
            s.schedule = schedule;
            s.clone()
        };
        crate::settings::save(&state, &settings)?;
        log::info!("scheduled Smart Care: {schedule:?}");
        Ok(ScheduleView {
            schedule,
            registered: smd_win::schedule::exists().unwrap_or(false),
        })
    })
    .await
}
