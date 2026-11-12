//! Applies and reverts the built-in tweaks: registry values (with a backup of each old value),
//! service start types (through `sc.exe`) and Store app removal for your account.

use std::io;
use std::process::Command;
use std::time::Duration;

use smd_core::tweaks::{
    Hive, MachineUndo, RawValue, RegData, RegPrevious, RegSetting, ServicePrevious, StartType,
    Tweak, TweakState,
};
use winreg::enums::{
    HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, KEY_SET_VALUE, KEY_WOW64_64KEY, RegType,
};
use winreg::{RegKey, RegValue};

fn root(h: Hive) -> RegKey {
    RegKey::predef(match h {
        Hive::Hkcu => HKEY_CURRENT_USER,
        Hive::Hklm => HKEY_LOCAL_MACHINE,
    })
}

fn regtype(v: u32) -> Option<RegType> {
    use RegType::*;
    Some(match v {
        0 => REG_NONE,
        1 => REG_SZ,
        2 => REG_EXPAND_SZ,
        3 => REG_BINARY,
        4 => REG_DWORD,
        7 => REG_MULTI_SZ,
        11 => REG_QWORD,
        _ => return None,
    })
}

fn open(h: Hive, key: &str, access: u32) -> io::Result<RegKey> {
    root(h).open_subkey_with_flags(key, access | KEY_WOW64_64KEY)
}

fn decode(raw: &RegValue) -> Option<RegData> {
    match raw.vtype {
        RegType::REG_DWORD if raw.bytes.len() >= 4 => Some(RegData::Dword(u32::from_le_bytes([
            raw.bytes[0],
            raw.bytes[1],
            raw.bytes[2],
            raw.bytes[3],
        ]))),
        RegType::REG_SZ | RegType::REG_EXPAND_SZ => {
            let wide: Vec<u16> = raw
                .bytes
                .as_chunks::<2>()
                .0
                .iter()
                .map(|c| u16::from_le_bytes(*c))
                .collect();
            let s = String::from_utf16_lossy(&wide);
            Some(RegData::Sz(s.trim_end_matches('\0').to_string()))
        }
        _ => None,
    }
}

/// The current value, or `None` if it is absent or of another type.
pub fn read(s: &RegSetting) -> Option<RegData> {
    let key = open(s.hive, &s.key, KEY_READ).ok()?;
    decode(&key.get_raw_value(&s.name).ok()?)
}

fn key_exists(h: Hive, key: &str) -> bool {
    open(h, key, KEY_READ).is_ok()
}

/// The highest missing key on the way to `key` (what a write would create).
fn first_missing(h: Hive, key: &str) -> Option<String> {
    if key_exists(h, key) {
        return None;
    }
    let parts: Vec<&str> = key.split('\\').collect();
    (1..=parts.len())
        .map(|n| parts[..n].join("\\"))
        .find(|k| !key_exists(h, k))
}

fn write(s: &RegSetting) -> io::Result<()> {
    let (key, _) =
        root(s.hive).create_subkey_with_flags(&s.key, KEY_SET_VALUE | KEY_WOW64_64KEY)?;
    match &s.value {
        RegData::Dword(v) => key.set_value(&s.name, v),
        RegData::Sz(v) => key.set_value(&s.name, v),
    }
}

fn apply_settings<'a>(
    settings: impl Iterator<Item = &'a RegSetting>,
) -> io::Result<Vec<RegPrevious>> {
    let mut done: Vec<RegPrevious> = Vec::new();
    for s in settings {
        let previous = open(s.hive, &s.key, KEY_READ)
            .ok()
            .and_then(|k| k.get_raw_value(&s.name).ok())
            .map(|v| RawValue {
                vtype: v.vtype as u32,
                bytes: v.bytes.to_vec(),
            });
        let created_key = first_missing(s.hive, &s.key);
        let prev = RegPrevious {
            hive: s.hive,
            key: s.key.clone(),
            name: s.name.clone(),
            previous,
            created_key,
        };
        if let Err(e) = write(s) {
            let _ = revert_registry(&done);
            return Err(e);
        }
        done.push(prev);
    }
    Ok(done)
}

fn delete_if_empty(h: Hive, key: &str) -> bool {
    let Ok(k) = open(h, key, KEY_READ) else {
        return true;
    };
    let empty = k.enum_keys().next().is_none() && k.enum_values().next().is_none();
    drop(k);
    empty
        && root(h)
            .delete_subkey_with_flags(key, KEY_WOW64_64KEY)
            .is_ok()
}

/// Puts the old values back, newest first, and removes keys the tweak created if they are empty.
pub fn revert_registry(prev: &[RegPrevious]) -> io::Result<()> {
    let mut first_error = None;
    for p in prev.iter().rev() {
        let r = (|| -> io::Result<()> {
            match &p.previous {
                Some(raw) => {
                    let (key, _) = root(p.hive)
                        .create_subkey_with_flags(&p.key, KEY_SET_VALUE | KEY_WOW64_64KEY)?;
                    let vtype = regtype(raw.vtype)
                        .ok_or_else(|| io::Error::other("unknown registry value type"))?;
                    key.set_raw_value(
                        &p.name,
                        &RegValue {
                            bytes: raw.bytes.clone().into(),
                            vtype,
                        },
                    )
                }
                None => match open(p.hive, &p.key, KEY_SET_VALUE) {
                    Ok(key) => match key.delete_value(&p.name) {
                        Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
                        _ => Ok(()),
                    },
                    Err(_) => Ok(()),
                },
            }
        })();
        if let Some(created) = &p.created_key {
            let mut key = p.key.clone();
            while delete_if_empty(p.hive, &key) && !key.eq_ignore_ascii_case(created) {
                match key.rfind('\\') {
                    Some(i) if i >= created.len() => key.truncate(i),
                    _ => break,
                }
            }
        }
        if let Err(e) = r {
            first_error.get_or_insert(e);
        }
    }
    first_error.map_or(Ok(()), Err)
}

const SERVICES: &str = r"SYSTEM\CurrentControlSet\Services";

fn service_start(name: &str) -> Option<(u32, bool)> {
    let key = open(Hive::Hklm, &format!(r"{SERVICES}\{name}"), KEY_READ).ok()?;
    let start: u32 = key.get_value("Start").ok()?;
    let delayed: u32 = key.get_value("DelayedAutostart").unwrap_or(0);
    Some((start, delayed == 1))
}

fn set_service(name: &str, start: u32, delayed: bool) -> io::Result<()> {
    let mode = match (start, delayed) {
        (2, true) => "delayed-auto",
        (2, false) => "auto",
        (3, _) => "demand",
        (4, _) => "disabled",
        _ => return Err(io::Error::other("unsupported start type")),
    };
    let sc =
        std::path::PathBuf::from(std::env::var("WINDIR").unwrap_or_else(|_| r"C:\Windows".into()))
            .join(r"System32\sc.exe");
    let mut cmd = Command::new(sc);
    cmd.args(["config", name, "start=", mode]);
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    let out = cmd.output()?;
    if !out.status.success() {
        return Err(io::Error::other(format!(
            "sc.exe could not change {name} ({})",
            out.status.code().unwrap_or(-1)
        )));
    }
    Ok(())
}

/// Machine-wide values and services (runs in the elevated helper).
pub fn apply_machine(t: &Tweak) -> io::Result<MachineUndo> {
    let registry = apply_settings(t.machine_registry())?;
    let mut services = Vec::new();
    for s in &t.services {
        let (start, delayed) = service_start(&s.name).ok_or_else(|| {
            io::Error::new(io::ErrorKind::NotFound, format!("service {}", s.name))
        })?;
        if let Err(e) = set_service(&s.name, s.start.registry_value(), false) {
            let undo = MachineUndo { registry, services };
            let _ = revert_machine(&undo);
            return Err(e);
        }
        services.push(ServicePrevious {
            name: s.name.clone(),
            start,
            delayed,
        });
    }
    Ok(MachineUndo { registry, services })
}

pub fn revert_machine(undo: &MachineUndo) -> io::Result<()> {
    let mut r = revert_registry(&undo.registry);
    for s in undo.services.iter().rev() {
        if let Err(e) = set_service(&s.name, s.start, s.delayed)
            && r.is_ok()
        {
            r = Err(e);
        }
    }
    r
}

/// Your account's values (runs in the app, never elevated).
pub fn apply_user(t: &Tweak) -> io::Result<Vec<RegPrevious>> {
    apply_settings(t.user_registry())
}

/// Current state of a registry/service tweak. App removals are checked with [`installed_appx`].
pub fn state(t: &Tweak) -> TweakState {
    let registry: Vec<_> = t.registry.iter().map(read).collect();
    let services: Vec<_> = t
        .services
        .iter()
        .map(|s| service_start(&s.name).and_then(|(v, _)| StartType::from_registry(v)))
        .collect();
    if t.services.iter().zip(&services).any(|(_, s)| s.is_none()) && t.registry.is_empty() {
        return TweakState::NotInstalled;
    }
    smd_core::tweaks::settings_state(t, &registry, &services)
}

const APPX_NAMES: &str = r#"
$ErrorActionPreference = 'Stop'
ConvertTo-Json -InputObject @(Get-AppxPackage | ForEach-Object { $_.Name } | Select-Object -Unique) -Compress
"#;

const APPX_REMOVE: &str = r#"
$ErrorActionPreference = 'Stop'
$p = @(Get-AppxPackage -Name $env:SMD_NAME)
foreach ($x in $p) { Remove-AppxPackage -Package $x.PackageFullName }
ConvertTo-Json -InputObject @($p | ForEach-Object { $_.PackageFamilyName } | Select-Object -Unique) -Compress
"#;

/// Names of the Store apps installed for your account.
pub fn installed_appx() -> io::Result<std::collections::HashSet<String>> {
    let names: Vec<String> =
        crate::powershell::run_json_with(APPX_NAMES, &[], Some(Duration::from_secs(60)))?;
    Ok(names.into_iter().map(|n| n.to_lowercase()).collect())
}

/// Removes a Store app for your account. Returns its package family names (for the Store link).
pub fn remove_appx(name: &str) -> io::Result<Vec<String>> {
    if !smd_core::apps::is_safe_appx_name(name) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "bad package name",
        ));
    }
    crate::powershell::run_json_with(
        APPX_REMOVE,
        &[("SMD_NAME", name)],
        Some(Duration::from_secs(300)),
    )
}

const RESTART_EXPLORER: &str = r#"
Stop-Process -Name explorer -Force -ErrorAction SilentlyContinue
Start-Sleep -Seconds 2
if (-not (Get-Process explorer -ErrorAction SilentlyContinue)) { Start-Process explorer.exe }
'true'
"#;

/// Restarts File Explorer so menu and taskbar changes show (open Explorer windows close).
pub fn restart_explorer() -> io::Result<()> {
    crate::powershell::run_text(RESTART_EXPLORER, &[], Some(Duration::from_secs(30))).map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setting(key: &str, name: &str, value: RegData) -> RegSetting {
        RegSetting {
            hive: Hive::Hkcu,
            key: key.into(),
            name: name.into(),
            value,
        }
    }

    #[test]
    fn apply_and_revert_restore_everything() {
        let base = format!(r"Software\SmdTest-{}", std::process::id());
        // An existing key with an existing value, and a new nested key.
        let (k, _) = root(Hive::Hkcu)
            .create_subkey_with_flags(&base, KEY_SET_VALUE)
            .unwrap();
        k.set_value("Kept", &7u32).unwrap();
        drop(k);
        let settings = [
            setting(&base, "Kept", RegData::Dword(0)),
            setting(
                &format!(r"{base}\New\Deeper"),
                "",
                RegData::Sz(String::new()),
            ),
        ];
        let prev = apply_settings(settings.iter()).unwrap();
        assert_eq!(read(&settings[0]), Some(RegData::Dword(0)));
        assert_eq!(read(&settings[1]), Some(RegData::Sz(String::new())));
        assert_eq!(
            prev[1].created_key.as_deref(),
            Some(format!(r"{base}\New").as_str())
        );

        revert_registry(&prev).unwrap();
        assert_eq!(read(&settings[0]), Some(RegData::Dword(7)));
        assert!(
            !key_exists(Hive::Hkcu, &format!(r"{base}\New")),
            "created keys are removed"
        );
        assert!(key_exists(Hive::Hkcu, &base), "keys that existed stay");
        root(Hive::Hkcu).delete_subkey_all(&base).unwrap();
    }
}
