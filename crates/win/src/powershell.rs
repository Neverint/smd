//! Runs fixed PowerShell scripts for official APIs that have no simpler binding.
//!
//! Rules (see CLAUDE.md): `-NoProfile -NonInteractive -ExecutionPolicy Bypass`,
//! JSON output parsed into typed structs, and scripts are `&'static str`:
//! user input is never concatenated into a command.

use serde::de::DeserializeOwned;
use std::io;
use std::process::Command;

/// Runs `script` and parses its stdout as JSON.
pub fn run_json<T: DeserializeOwned>(script: &'static str) -> io::Result<T> {
    let mut cmd = Command::new("powershell.exe");
    cmd.args([
        "-NoProfile",
        "-NonInteractive",
        "-ExecutionPolicy",
        "Bypass",
        "-Command",
        script,
    ]);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let out = cmd.output()?;
    if !out.status.success() {
        return Err(io::Error::other(format!(
            "PowerShell exited with {}: {}",
            out.status,
            String::from_utf8_lossy(&out.stderr).trim()
        )));
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let text = text.trim().trim_start_matches('\u{feff}');
    serde_json::from_str(text).map_err(|e| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("unexpected PowerShell output: {e}"),
        )
    })
}
