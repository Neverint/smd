//! The app's side of the elevated helper: starts it with UAC when an admin action needs it,
//! keeps the session for the batch, and maps its errors.

use serde::de::DeserializeOwned;
use smd_win::elevate::Client;
use smd_win::helper::Op;

use crate::error::{AppError, AppResult};
use crate::state::{AppState, lock};

fn map(e: std::io::Error) -> AppError {
    match e.kind() {
        std::io::ErrorKind::PermissionDenied => AppError::AccessDenied(e.to_string()),
        std::io::ErrorKind::NotFound => AppError::NotFound(e.to_string()),
        _ => AppError::Internal(format!("admin helper: {e}")),
    }
}

/// Runs one operation in the elevated helper, starting it (UAC prompt) if needed.
/// A helper that exited (idle for 10 minutes) is started again once.
pub fn call<T: DeserializeOwned>(state: &AppState, op: &Op) -> AppResult<T> {
    let mut slot = lock(&state.helper)?;
    for attempt in 0..2 {
        if slot.as_ref().is_none_or(|c| !c.is_alive()) {
            let exe = std::env::current_exe()?;
            log::info!("starting the admin helper (UAC prompt)");
            *slot = Some(Client::start(&exe).map_err(map)?);
        }
        let client = slot.as_mut().expect("started above");
        match client.call::<T>(op) {
            Ok(v) => return Ok(v),
            Err(e) if e.kind() == std::io::ErrorKind::BrokenPipe && attempt == 0 => {
                *slot = None;
            }
            Err(e) => {
                log::warn!("admin helper {op:?} failed: {e}");
                return Err(map(e));
            }
        }
    }
    Err(AppError::Internal("the admin helper stopped".into()))
}

/// True while an admin session is open (no new UAC prompt needed).
pub fn is_running(state: &AppState) -> bool {
    match state.helper.try_lock() {
        Ok(s) => s.as_ref().is_some_and(Client::is_alive),
        // Busy with an admin operation right now.
        Err(std::sync::TryLockError::WouldBlock) => true,
        Err(_) => false,
    }
}

/// Closes the admin session (the helper exits).
pub fn stop(state: &AppState) {
    if let Ok(mut s) = lock(&state.helper) {
        *s = None;
    }
}

/// `smd.exe --helper <pipe> <pid>`: runs the helper instead of the app. Returns the exit code.
pub fn helper_main() -> Option<i32> {
    let args: Vec<String> = std::env::args().collect();
    let (pipe, pid) = smd_win::helper::helper_args(&args)?;
    Some(smd_win::helper::serve(&pipe, pid))
}
