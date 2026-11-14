//! DEVLOG: developer data log, for the road to 1.0 only. See docs/DEVLOG.md.
//!
//! Every app session writes one JSON Lines file to `%LOCALAPPDATA%\io.github.neverint.smd\devlog`:
//! session start (version, build, Windows), every backend log line, every command the UI calls
//! (with its time and error), pages, clicks, UI errors, panics and the session end. A file with no
//! `end` line means the app was killed or crashed. Nothing leaves the PC.
//!
//! Remove with `npm run devlog:remove` before 1.0 (a test below fails from 1.0 on).
//! Turn off for one run with the environment variable `SMD_DEVLOG=0`.

use serde_json::{Value, json};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

/// Sessions kept on disk; older files are removed at start.
const KEEP_SESSIONS: usize = 100;
/// A session file stops growing at this size (a `truncated` line says so).
const MAX_SESSION_BYTES: u64 = 20 * 1024 * 1024;

struct Session {
    file: File,
    written: u64,
    truncated: bool,
}

static SESSION: OnceLock<Mutex<Session>> = OnceLock::new();

pub fn dir() -> Option<PathBuf> {
    let base = std::env::var_os("LOCALAPPDATA")?;
    Some(
        PathBuf::from(base)
            .join("io.github.neverint.smd")
            .join("devlog"),
    )
}

fn now_ms() -> i64 {
    crate::store::now_ms()
}

/// Opens this session's file and records the start. Call once, first thing in `run`.
pub fn start() {
    if std::env::var("SMD_DEVLOG").is_ok_and(|v| v == "0") {
        return;
    }
    let Some(dir) = dir() else { return };
    if fs::create_dir_all(&dir).is_err() {
        return;
    }
    prune(&dir);
    let path = dir.join(format!("{}-{}.jsonl", now_ms(), std::process::id()));
    let Ok(file) = OpenOptions::new().create(true).append(true).open(&path) else {
        return;
    };
    let _ = SESSION.set(Mutex::new(Session {
        file,
        written: 0,
        truncated: false,
    }));
    install_panic_hook();
    let exe = std::env::current_exe().ok();
    let exe_meta = exe.as_ref().and_then(|p| fs::metadata(p).ok());
    event(
        "start",
        json!({
            "version": env!("CARGO_PKG_VERSION"),
            "build": if cfg!(debug_assertions) { "debug" } else { "release" },
            "exe": exe.as_ref().map(|p| p.display().to_string()),
            "exeBytes": exe_meta.as_ref().map(fs::Metadata::len),
            "exeModifiedMs": exe_meta
                .and_then(|m| m.modified().ok())
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_millis() as i64),
            "args": std::env::args().skip(1).collect::<Vec<_>>(),
            "windows": windows_version(),
            "pid": std::process::id(),
            "tzOffsetMin": tz_offset_minutes(),
        }),
    );
}

/// Records the clean end of a session (`RunEvent::Exit`).
pub fn on_run_event(event_: &tauri::RunEvent) {
    if let tauri::RunEvent::Exit = event_ {
        event("end", json!({}));
    }
}

/// Appends one line `{"t": <unix ms>, "k": kind, ...data}` and flushes it, so a crash keeps it.
pub fn event(kind: &str, data: Value) {
    let Some(session) = SESSION.get() else { return };
    let mut line = match data {
        Value::Object(map) => map,
        other => {
            let mut m = serde_json::Map::new();
            m.insert("data".into(), other);
            m
        }
    };
    line.insert("t".into(), json!(now_ms()));
    line.insert("k".into(), json!(kind));
    let Ok(mut text) = serde_json::to_string(&line) else {
        return;
    };
    text.push('\n');
    let Ok(mut s) = session.lock() else { return };
    if s.truncated {
        return;
    }
    if s.written + text.len() as u64 > MAX_SESSION_BYTES {
        s.truncated = true;
        let _ = writeln!(s.file, "{{\"t\":{},\"k\":\"truncated\"}}", now_ms());
        return;
    }
    if s.file.write_all(text.as_bytes()).is_ok() {
        s.written += text.len() as u64;
        let _ = s.file.flush();
    }
}

/// Backend log lines (info and above, the plugin's level) also go to the session file.
pub fn log_target() -> tauri_plugin_log::Target {
    use tauri_plugin_log::fern;
    let dispatch = fern::Dispatch::new().chain(fern::Output::call(|record| {
        event(
            "log",
            json!({
                "level": record.level().as_str(),
                "target": record.target(),
                "msg": strip_prefix(&record.args().to_string()),
            }),
        );
    }));
    tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::Dispatch(dispatch))
}

/// The log plugin has already added `[date][time][LEVEL][target] `: the line has its own time.
fn strip_prefix(msg: &str) -> String {
    match msg.split_once("] ") {
        Some((head, rest)) if head.starts_with('[') => rest.to_string(),
        _ => msg.to_string(),
    }
}

/// Events from the UI (`src/dev/devlog.ts`), sent in batches.
#[tauri::command]
pub fn devlog_write(events: Vec<Value>) {
    for e in events {
        let kind = e
            .get("k")
            .and_then(Value::as_str)
            .unwrap_or("ui")
            .to_string();
        let mut data = e;
        if let Value::Object(m) = &mut data {
            m.remove("k");
            // Keep the UI's own time as `ut`; `t` is when the backend wrote it.
            if let Some(t) = m.remove("t") {
                m.insert("ut".into(), t);
            }
        }
        event(&format!("ui.{kind}"), data);
    }
}

/// Panics abort the release build (`panic = "abort"`), so write them before the process ends.
fn install_panic_hook() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let msg = info
            .payload()
            .downcast_ref::<&str>()
            .map(|s| (*s).to_string())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_default();
        event(
            "panic",
            json!({
                "msg": msg,
                "at": info.location().map(|l| format!("{}:{}", l.file(), l.line())),
                "thread": std::thread::current().name().map(str::to_string),
                "backtrace": std::backtrace::Backtrace::force_capture().to_string(),
            }),
        );
        previous(info);
    }));
}

/// Removes the oldest session files, keeping `KEEP_SESSIONS`.
fn prune(dir: &std::path::Path) {
    let Ok(read) = fs::read_dir(dir) else { return };
    let mut files: Vec<PathBuf> = read
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "jsonl"))
        .collect();
    // Names start with the unix time in ms, all the same width for centuries: name order = age.
    files.sort();
    let extra = files.len().saturating_sub(KEEP_SESSIONS - 1);
    for f in files.into_iter().take(extra) {
        let _ = fs::remove_file(f);
    }
}

#[cfg(windows)]
fn windows_version() -> Value {
    use winreg::RegKey;
    use winreg::enums::HKEY_LOCAL_MACHINE;
    let Ok(k) = RegKey::predef(HKEY_LOCAL_MACHINE)
        .open_subkey(r"SOFTWARE\Microsoft\Windows NT\CurrentVersion")
    else {
        return Value::Null;
    };
    let s = |name: &str| k.get_value::<String, _>(name).ok();
    let build = s("CurrentBuild");
    // ProductName still says "Windows 10" on Windows 11; the build number tells them apart.
    let windows_11 = build
        .as_deref()
        .and_then(|b| b.parse::<u32>().ok())
        .is_some_and(|b| b >= 22000);
    let product = s("ProductName").map(|p| {
        if windows_11 {
            p.replace("Windows 10", "Windows 11")
        } else {
            p
        }
    });
    json!({
        "product": product,
        "display": s("DisplayVersion"),
        "build": build,
        "ubr": k.get_value::<u32, _>("UBR").ok(),
        "edition": s("EditionID"),
    })
}

#[cfg(not(windows))]
fn windows_version() -> Value {
    Value::Null
}

/// Local time offset from UTC in minutes, so the summary script can show local times.
#[cfg(windows)]
fn tz_offset_minutes() -> Option<i32> {
    use winreg::RegKey;
    use winreg::enums::HKEY_LOCAL_MACHINE;
    let k = RegKey::predef(HKEY_LOCAL_MACHINE)
        .open_subkey(r"SYSTEM\CurrentControlSet\Control\TimeZoneInformation")
        .ok()?;
    // "UTC = local + bias", stored as a u32 that holds an i32.
    let bias = k.get_value::<u32, _>("ActiveTimeBias").ok()? as i32;
    Some(-bias)
}

#[cfg(not(windows))]
fn tz_offset_minutes() -> Option<i32> {
    None
}

#[cfg(test)]
mod tests {
    #[test]
    fn devlog_is_removed_before_1_0() {
        let major: u32 = env!("CARGO_PKG_VERSION")
            .split('.')
            .next()
            .and_then(|m| m.parse().ok())
            .unwrap_or(0);
        assert!(
            major < 1,
            "The developer data log must not ship in 1.0: run `npm run devlog:remove` (docs/DEVLOG.md)."
        );
    }

    #[test]
    fn strips_the_log_plugin_prefix() {
        assert_eq!(
            super::strip_prefix("[2026-11-14][15:09:10][INFO][smd_lib] window effect: Mica"),
            "window effect: Mica"
        );
        assert_eq!(super::strip_prefix("plain"), "plain");
    }

    #[test]
    fn prune_keeps_the_newest_sessions() {
        let dir = tempfile::tempdir().unwrap();
        for i in 0..(super::KEEP_SESSIONS + 5) {
            std::fs::write(
                dir.path()
                    .join(format!("{:013}-1.jsonl", 1_700_000_000_000u64 + i as u64)),
                "",
            )
            .unwrap();
        }
        std::fs::write(dir.path().join("notes.txt"), "").unwrap();
        super::prune(dir.path());
        let mut left: Vec<String> = std::fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        left.sort();
        // One slot is kept free for the session that is starting.
        assert_eq!(left.len(), super::KEEP_SESSIONS); // KEEP - 1 sessions + notes.txt
        assert!(left.contains(&"notes.txt".to_string()));
        assert!(!left.contains(&format!("{:013}-1.jsonl", 1_700_000_000_000u64)));
    }
}
