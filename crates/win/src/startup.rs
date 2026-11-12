//! Startup items (Run keys and Startup folders) and turning them off/on through
//! `StartupApproved`, exactly like Task Manager. The Run entries themselves are never changed.

use serde::Serialize;
use smd_core::startup::Source;
use std::io;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StartupItem {
    /// `source:name`, stable across scans.
    pub id: String,
    pub name: String,
    pub command: String,
    pub program: Option<String>,
    pub publisher: Option<String>,
    pub microsoft: bool,
    pub source: Source,
    pub enabled: bool,
    /// Machine-wide: needs the admin helper (Phase 4) to change.
    pub needs_admin: bool,
}

/// Apps that update with Squirrel (Discord, Slack...) start through `Update.exe --processStart App.exe`.
/// The launcher is published by GitHub; the real program is `app-<version>\App.exe` next to it.
pub fn squirrel_target(program: &str, command: &str) -> Option<String> {
    let path = std::path::Path::new(program);
    if !path
        .file_name()?
        .to_string_lossy()
        .eq_ignore_ascii_case("update.exe")
    {
        return None;
    }
    let lower = command.to_ascii_lowercase();
    let i = lower.find("--processstart")?;
    let exe = command[i + "--processstart".len()..]
        .split_whitespace()
        .next()?
        .trim_matches('"');
    if exe.is_empty() || exe.contains(['\\', '/']) {
        return None;
    }
    let mut versions: Vec<std::path::PathBuf> = std::fs::read_dir(path.parent()?)
        .ok()?
        .flatten()
        .filter(|e| {
            e.file_name()
                .to_string_lossy()
                .to_ascii_lowercase()
                .starts_with("app-")
        })
        .map(|e| e.path().join(exe))
        .filter(|p| p.is_file())
        .collect();
    // Newest install wins ("app-1.0.10" sorts before "app-1.0.9" as text).
    versions.sort_by_key(|p| std::fs::metadata(p).and_then(|m| m.modified()).ok());
    versions.pop().map(|p| p.to_string_lossy().into_owned())
}

pub fn item_id(source: Source, name: &str) -> String {
    format!(
        "{}:{name}",
        serde_json::to_value(source)
            .ok()
            .and_then(|v| v.as_str().map(str::to_string))
            .unwrap_or_default()
    )
}

#[cfg(windows)]
mod imp {
    use super::*;
    use smd_core::startup::{approved_value, filetime_from_unix_ms, is_enabled};
    use winreg::RegKey;
    use winreg::enums::{
        HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, KEY_SET_VALUE, REG_BINARY,
    };

    const RUN: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
    const RUN32: &str = r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Run";
    const APPROVED: &str = r"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved";

    struct Place {
        source: Source,
        machine: bool,
        approved_sub: &'static str,
    }

    fn place(source: Source) -> Place {
        match source {
            Source::UserRun => Place {
                source,
                machine: false,
                approved_sub: "Run",
            },
            Source::MachineRun => Place {
                source,
                machine: true,
                approved_sub: "Run",
            },
            Source::MachineRun32 => Place {
                source,
                machine: true,
                approved_sub: "Run32",
            },
            Source::UserFolder => Place {
                source,
                machine: false,
                approved_sub: "StartupFolder",
            },
            Source::CommonFolder => Place {
                source,
                machine: true,
                approved_sub: "StartupFolder",
            },
        }
    }

    fn hive(machine: bool) -> RegKey {
        RegKey::predef(if machine {
            HKEY_LOCAL_MACHINE
        } else {
            HKEY_CURRENT_USER
        })
    }

    fn approved_bytes(p: &Place, name: &str) -> Option<Vec<u8>> {
        let key = hive(p.machine)
            .open_subkey_with_flags(format!(r"{APPROVED}\{}", p.approved_sub), KEY_READ)
            .ok()?;
        key.get_raw_value(name).ok().map(|v| v.bytes.to_vec())
    }

    fn make(p: &Place, name: String, command: String) -> StartupItem {
        let program = crate::shell::program_of(&command)
            .map(|prog| super::squirrel_target(&prog, &command).unwrap_or(prog));
        let publisher = program.as_deref().and_then(crate::shell::file_company);
        let microsoft = publisher
            .as_deref()
            .is_some_and(smd_core::apps::is_microsoft);
        let enabled = is_enabled(approved_bytes(p, &name).as_deref());
        StartupItem {
            id: item_id(p.source, &name),
            name,
            command,
            program,
            publisher,
            microsoft,
            source: p.source,
            enabled,
            needs_admin: p.source.needs_admin(),
        }
    }

    fn run_key(out: &mut Vec<StartupItem>, source: Source, path: &str) {
        let p = place(source);
        let Ok(key) = hive(p.machine).open_subkey_with_flags(path, KEY_READ) else {
            return;
        };
        for (name, _) in key.enum_values().flatten() {
            if name.is_empty() {
                continue;
            }
            if let Ok(command) = key.get_value::<String, _>(&name) {
                out.push(make(&p, name, command));
            }
        }
    }

    fn folder(out: &mut Vec<StartupItem>, source: Source, dir: Option<std::path::PathBuf>) {
        let p = place(source);
        let Some(dir) = dir else { return };
        let Ok(entries) = std::fs::read_dir(&dir) else {
            return;
        };
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if name.eq_ignore_ascii_case("desktop.ini") || !e.file_type().is_ok_and(|t| t.is_file())
            {
                continue;
            }
            let path = e.path().to_string_lossy().into_owned();
            let mut item = make(&p, name, path.clone());
            // A shortcut's own file has no publisher; keep the shortcut path as the program.
            item.program = Some(path);
            out.push(item);
        }
    }

    pub fn list() -> Vec<StartupItem> {
        let mut out = Vec::new();
        run_key(&mut out, Source::UserRun, RUN);
        run_key(&mut out, Source::MachineRun, RUN);
        run_key(&mut out, Source::MachineRun32, RUN32);
        let appdata = std::env::var("APPDATA").ok().map(std::path::PathBuf::from);
        let progdata = std::env::var("ProgramData")
            .ok()
            .map(std::path::PathBuf::from);
        folder(
            &mut out,
            Source::UserFolder,
            appdata.map(|a| a.join(r"Microsoft\Windows\Start Menu\Programs\Startup")),
        );
        folder(
            &mut out,
            Source::CommonFolder,
            progdata.map(|a| a.join(r"Microsoft\Windows\Start Menu\Programs\StartUp")),
        );
        out.sort_by_key(|a| a.name.to_lowercase());
        out
    }

    /// Turns an item off or on. Returns the previous state. Machine-wide items are refused
    /// unless `allow_machine` (the elevated helper).
    pub fn set_enabled_any(
        id: &str,
        enabled: bool,
        now_ms: i64,
        allow_machine: bool,
    ) -> io::Result<bool> {
        let item = list()
            .into_iter()
            .find(|i| i.id == id)
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, format!("startup item {id}")))?;
        if item.needs_admin && !allow_machine {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "machine-wide startup items need admin rights",
            ));
        }
        let p = place(item.source);
        let (key, _) = hive(p.machine)
            .create_subkey_with_flags(format!(r"{APPROVED}\{}", p.approved_sub), KEY_SET_VALUE)?;
        let value = approved_value(enabled, filetime_from_unix_ms(now_ms));
        key.set_raw_value(
            &item.name,
            &winreg::RegValue {
                bytes: value.to_vec().into(),
                vtype: REG_BINARY,
            },
        )?;
        Ok(item.enabled)
    }
}

#[cfg(not(windows))]
mod imp {
    use super::*;
    pub fn list() -> Vec<StartupItem> {
        Vec::new()
    }
    pub fn set_enabled_any(
        _id: &str,
        _enabled: bool,
        _now_ms: i64,
        _allow_machine: bool,
    ) -> io::Result<bool> {
        Err(io::Error::new(io::ErrorKind::Unsupported, "Windows only"))
    }
}

pub use imp::*;

/// Turns one of your own startup items off or on. Returns the previous state.
pub fn set_enabled(id: &str, enabled: bool, now_ms: i64) -> io::Result<bool> {
    set_enabled_any(id, enabled, now_ms, false)
}

#[cfg(test)]
mod squirrel_tests {
    #[test]
    fn finds_the_real_program_behind_update_exe() {
        let dir = tempfile::tempdir().unwrap();
        let update = dir.path().join("Update.exe");
        std::fs::write(&update, b"x").unwrap();
        for v in ["app-1.0.1", "app-1.0.9"] {
            std::fs::create_dir_all(dir.path().join(v)).unwrap();
            std::fs::write(dir.path().join(v).join("Discord.exe"), b"x").unwrap();
            std::thread::sleep(std::time::Duration::from_millis(30));
        }
        let cmd = format!(
            r#""{}" --processStart Discord.exe --process-start-args "--start-minimized""#,
            update.display()
        );
        let found = super::squirrel_target(&update.to_string_lossy(), &cmd).unwrap();
        assert!(
            found.ends_with(r"app-1.0.9\Discord.exe") || found.ends_with("app-1.0.9/Discord.exe"),
            "{found}"
        );
        assert_eq!(super::squirrel_target(r"C:\x\other.exe", &cmd), None);
        assert_eq!(
            super::squirrel_target(
                &update.to_string_lossy(),
                "Update.exe --processStart ..\\evil.exe"
            ),
            None
        );
    }
}

#[cfg(all(test, windows))]
mod tests {
    #[test]
    fn lists_without_changing_anything() {
        let items = super::list();
        for i in &items {
            assert!(!i.id.is_empty() && !i.name.is_empty());
            assert_eq!(i.needs_admin, i.source.needs_admin());
        }
        assert_eq!(
            super::item_id(smd_core::startup::Source::UserRun, "X"),
            "userRun:X"
        );
    }
}
