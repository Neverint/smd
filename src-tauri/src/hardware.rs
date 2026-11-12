//! Hardware Health: system details, disks, memory, battery, and live sensors.

use serde::Serialize;
use smd_core::hardware::BatteryInfo;
use smd_win::disks::PhysicalDisk;
use smd_win::hardware::{SensorHelper, SensorSnapshot, SystemInfo, helper_path};
use smd_win::system::MemoryInfo;
use tauri::{AppHandle, Manager};

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
pub struct HardwareView {
    pub system: Option<SystemInfo>,
    pub disks: Option<Vec<PhysicalDisk>>,
    pub memory: Option<MemoryInfo>,
    /// Empty on desktops.
    pub batteries: Vec<BatteryInfo>,
    pub has_battery: bool,
    /// The PawnIO driver is installed (CPU temperatures and fans with admin rights).
    pub pawnio: bool,
}

fn pawnio_installed() -> bool {
    use winreg::RegKey;
    use winreg::enums::{HKEY_LOCAL_MACHINE, KEY_READ};
    RegKey::predef(HKEY_LOCAL_MACHINE)
        .open_subkey_with_flags(r"SYSTEM\CurrentControlSet\Services\PawnIO", KEY_READ)
        .is_ok()
}

#[tauri::command]
pub async fn hardware_info(app: AppHandle) -> AppResult<HardwareView> {
    blocking(move || {
        let state = app.state::<AppState>();
        let temp = state.data_dir.join("tmp");
        let (system, disks, batteries) = std::thread::scope(|s| {
            let sys = s.spawn(smd_win::hardware::system_info);
            let d = s.spawn(smd_win::disks::physical_disks);
            let b = smd_win::hardware::batteries(&temp);
            (
                sys.join().ok().and_then(|r| r.ok()),
                d.join().ok().and_then(|r| r.ok()),
                b.unwrap_or_default(),
            )
        });
        Ok(HardwareView {
            system,
            disks,
            memory: smd_win::system::memory().ok(),
            batteries,
            has_battery: smd_win::system::has_battery(),
            pawnio: pawnio_installed(),
        })
    })
    .await
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SensorsView {
    pub running: bool,
    /// The helper stopped: usually the .NET 8 runtime is missing.
    pub exit_code: Option<i32>,
    pub snapshot: Option<SensorSnapshot>,
}

/// Starts the sensor helper (if not running) and returns its latest reading.
#[tauri::command]
pub fn sensors_poll(app: AppHandle, state: tauri::State<'_, AppState>) -> AppResult<SensorsView> {
    let mut slot = lock(&state.sensors)?;
    if slot.is_none() {
        let dir = app.path().resource_dir()?;
        let exe = helper_path(&dir);
        match SensorHelper::start(&exe) {
            Ok(h) => *slot = Some(h),
            Err(e) => {
                log::warn!("sensor helper did not start: {e}");
                return Err(AppError::Unsupported(e.to_string()));
            }
        }
    }
    let helper = slot.as_mut().expect("just started");
    if let Some(code) = helper.exited() {
        *slot = None;
        return Ok(SensorsView {
            running: false,
            exit_code: Some(code),
            snapshot: None,
        });
    }
    Ok(SensorsView {
        running: true,
        exit_code: None,
        snapshot: helper.latest(),
    })
}

/// Stops the sensor helper (leaving the page). Keeps idle memory and CPU low.
#[tauri::command]
pub fn sensors_stop(state: tauri::State<'_, AppState>) -> AppResult<()> {
    *lock(&state.sensors)? = None;
    Ok(())
}

/// SMART counters (temperature, wear, hours, errors) through the admin helper.
#[tauri::command]
pub async fn disk_reliability(app: AppHandle) -> AppResult<Vec<smd_win::helper::DiskReliability>> {
    blocking(move || {
        let state = app.state::<AppState>();
        crate::helper::call(&state, &smd_win::helper::Op::DiskReliability)
    })
    .await
}

/// Sensors read with admin rights (CPU temperature and fans need the PawnIO driver too).
#[tauri::command]
pub async fn sensors_admin_poll(app: AppHandle) -> AppResult<SensorsView> {
    blocking(move || {
        let state = app.state::<AppState>();
        // The normal helper is not needed while the admin one runs.
        *lock(&state.sensors)? = None;
        let snap: Option<SensorSnapshot> =
            crate::helper::call(&state, &smd_win::helper::Op::SensorsPoll)?;
        *lock(&state.admin_sensors)? = snap.clone();
        Ok(SensorsView {
            running: true,
            exit_code: None,
            snapshot: snap,
        })
    })
    .await
}

#[tauri::command]
pub async fn sensors_admin_stop(app: AppHandle) -> AppResult<()> {
    blocking(move || {
        let state = app.state::<AppState>();
        *lock(&state.admin_sensors)? = None;
        if crate::helper::is_running(&state) {
            crate::helper::call::<bool>(&state, &smd_win::helper::Op::SensorsStop)?;
        }
        Ok(())
    })
    .await
}
