//! Runs fixed PowerShell scripts for official APIs that have no simpler binding.
//!
//! Rules (see CLAUDE.md): `-NoProfile -NonInteractive -ExecutionPolicy Bypass`,
//! JSON output parsed into typed structs, and scripts are `&'static str`.
//! Values a script needs (a package id) are passed as `SMD_*` environment
//! variables and read with `$env:SMD_...`, never pasted into the command.

use serde::de::DeserializeOwned;
use std::io::{self, Read};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Runs `script` and parses its stdout as JSON.
pub fn run_json<T: DeserializeOwned>(script: &'static str) -> io::Result<T> {
    run_json_with(script, &[], None)
}

/// Runs `script` with `SMD_*` variables, optionally killing it after `timeout`.
pub fn run_json_with<T: DeserializeOwned>(
    script: &'static str,
    vars: &[(&str, &str)],
    timeout: Option<Duration>,
) -> io::Result<T> {
    let text = run_text(script, vars, timeout)?;
    let text = text.trim().trim_start_matches('\u{feff}');
    serde_json::from_str(text).map_err(|e| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("unexpected PowerShell output: {e}"),
        )
    })
}

/// Runs `script` and returns its stdout.
pub fn run_text(
    script: &'static str,
    vars: &[(&str, &str)],
    timeout: Option<Duration>,
) -> io::Result<String> {
    for (k, _) in vars {
        if !k.starts_with("SMD_") {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "script variables must start with SMD_",
            ));
        }
    }
    let mut cmd = Command::new("powershell.exe");
    cmd.args([
        "-NoProfile",
        "-NonInteractive",
        "-ExecutionPolicy",
        "Bypass",
        "-Command",
        script,
    ])
    .stdin(Stdio::null())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped());
    for (k, v) in vars {
        cmd.env(k, v);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let mut child = cmd.spawn()?;
    // Read the pipes on their own threads so a large output never blocks the child.
    let mut stdout = child.stdout.take().expect("piped");
    let mut stderr = child.stderr.take().expect("piped");
    let out_t = std::thread::spawn(move || {
        let mut b = Vec::new();
        let _ = stdout.read_to_end(&mut b);
        b
    });
    let err_t = std::thread::spawn(move || {
        let mut b = Vec::new();
        let _ = stderr.read_to_end(&mut b);
        b
    });

    let started = Instant::now();
    let status = loop {
        if let Some(s) = child.try_wait()? {
            break s;
        }
        if timeout.is_some_and(|t| started.elapsed() > t) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "PowerShell took too long",
            ));
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    let out = out_t.join().unwrap_or_default();
    let err = err_t.join().unwrap_or_default();
    if !status.success() {
        return Err(io::Error::other(format!(
            "PowerShell exited with {status}: {}",
            String::from_utf8_lossy(&err).trim()
        )));
    }
    Ok(String::from_utf8_lossy(&out).into_owned())
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn passes_values_through_the_environment() {
        let v: String = run_json_with(
            "ConvertTo-Json -InputObject $env:SMD_TEST",
            &[("SMD_TEST", "a'b\"; Remove-Item x")],
            None,
        )
        .unwrap();
        assert_eq!(v, "a'b\"; Remove-Item x", "the value is data, never code");
        assert!(run_text("1", &[("PATH", "x")], None).is_err());
    }

    #[test]
    fn times_out() {
        let e = run_text(
            "Start-Sleep -Seconds 20",
            &[],
            Some(Duration::from_millis(500)),
        )
        .unwrap_err();
        assert_eq!(e.kind(), io::ErrorKind::TimedOut);
    }
}
