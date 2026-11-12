//! The elevated helper (`smd.exe --helper <pipe> <app pid>`): a short-lived admin process that
//! runs only the typed operations below, each checked against the built-in catalogs.
//!
//! It exits when the app closes the pipe, on `Exit`, or after 10 minutes without a request.

use std::io::{self, BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use smd_core::cleanup::builtin_rules;
use smd_core::tweaks::MachineUndo;

use crate::cleanup::{Category, Destination, Outcome, ScanEnv, remove_items_to, scan_rule};
use crate::elevate::{Request, Response};
use crate::hardware::{SensorHelper, SensorSnapshot};

const IDLE_EXIT: Duration = Duration::from_secs(600);

/// Every operation the helper accepts. Nothing else can run elevated.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Op {
    Ping,
    Exit,
    /// System Restore point on the system drive (one per 24 h unless Windows allows more).
    RestorePoint {
        description: String,
    },
    /// Turns on System Protection for the system drive.
    EnableRestore,
    /// Scans the admin cleanup rules (Windows temp, update cache...).
    CleanupScan {
        rule_ids: Vec<String>,
        exclusions: Vec<String>,
    },
    /// Removes items of one admin rule. Only paths the helper's own new scan finds are removed.
    CleanupRemove {
        rule_id: String,
        paths: Vec<String>,
        exclusions: Vec<String>,
    },
    /// Moves leftover folders of an uninstalled app (Program Files, ProgramData) to the Recycle
    /// Bin. Only folders the helper's own new search finds, never shared or installed ones.
    LeftoversRemove {
        app: crate::apps::InstalledApp,
        paths: Vec<String>,
    },
    /// Turns a machine-wide startup item off or on (`StartupApproved`, like Task Manager).
    StartupSet {
        id: String,
        enabled: bool,
    },
    /// Applies the machine-wide part of a built-in tweak.
    TweakApply {
        id: String,
    },
    /// Puts back the machine-wide part of a tweak (only values and services the tweak declares).
    TweakRevert {
        id: String,
        undo: MachineUndo,
    },
    /// SMART counters per disk (`Get-StorageReliabilityCounter`).
    DiskReliability,
    /// Latest reading of the sensor helper running with admin rights.
    SensorsPoll,
    SensorsStop,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(
    tag = "status",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum RestorePoint {
    /// A new restore point was made.
    Created {
        time_ms: Option<i64>,
    },
    /// Windows made none because one is less than 24 hours old: that one covers this batch.
    Recent {
        time_ms: Option<i64>,
    },
    /// System Protection is off for the system drive.
    Off,
    Failed {
        message: String,
    },
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiskReliability {
    pub serial: String,
    pub name: String,
    pub temperature: Option<u32>,
    pub temperature_max: Option<u32>,
    /// Percent of rated life used (SSDs).
    pub wear: Option<u32>,
    pub power_on_hours: Option<u64>,
    pub read_errors: Option<u64>,
    pub write_errors: Option<u64>,
}

pub fn same_file(a: &Path, b: &Path) -> bool {
    match (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
        (Ok(a), Ok(b)) => a
            .to_string_lossy()
            .eq_ignore_ascii_case(&b.to_string_lossy()),
        _ => false,
    }
}

const RESTORE_POINT: &str = r#"
$ErrorActionPreference = 'Stop'
function Last { Get-ComputerRestorePoint -ErrorAction SilentlyContinue | Sort-Object SequenceNumber | Select-Object -Last 1 }
function Ms($p) { if ($p) { ([DateTimeOffset]$p.ConvertToDateTime($p.CreationTime)).ToUnixTimeMilliseconds() } else { $null } }
$letter = $env:SystemDrive.TrimEnd(':')
$spp = (Get-ItemProperty 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion\SPP\Clients' -ErrorAction SilentlyContinue).'{09F7EDC5-294E-4180-AF6A-FB0E6A0E9513}'
$on = [bool](@($spp) | Where-Object { "$_" -like "*($letter%3A)*" })
$before = Last
try {
  Checkpoint-Computer -Description $env:SMD_DESC -RestorePointType MODIFY_SETTINGS -WarningAction SilentlyContinue -WarningVariable warn
} catch {
  if (-not $on) { [pscustomobject]@{ status = 'off' } | ConvertTo-Json -Compress; exit }
  [pscustomobject]@{ status = 'failed'; message = $_.Exception.Message } | ConvertTo-Json -Compress; exit
}
$after = Last
if ($after -and (-not $before -or $after.SequenceNumber -ne $before.SequenceNumber)) {
  [pscustomobject]@{ status = 'created'; timeMs = (Ms $after) } | ConvertTo-Json -Compress
} elseif (-not $on) {
  [pscustomobject]@{ status = 'off' } | ConvertTo-Json -Compress
} elseif ($warn -or $after) {
  [pscustomobject]@{ status = 'recent'; timeMs = (Ms $after) } | ConvertTo-Json -Compress
} else {
  [pscustomobject]@{ status = 'failed'; message = 'no restore point was made' } | ConvertTo-Json -Compress
}
"#;

const ENABLE_RESTORE: &str = r#"
$ErrorActionPreference = 'Stop'
Enable-ComputerRestore -Drive "$env:SystemDrive\"
'true'
"#;

const DISK_RELIABILITY: &str = r#"
$ErrorActionPreference = 'SilentlyContinue'
$out = @(Get-PhysicalDisk | ForEach-Object {
  $r = $_ | Get-StorageReliabilityCounter
  [pscustomobject]@{
    serial = "$($_.SerialNumber)".Trim(); name = $_.FriendlyName
    temperature = $r.Temperature; temperatureMax = $r.TemperatureMax; wear = $r.Wear
    powerOnHours = $r.PowerOnHours; readErrors = $r.ReadErrorsUncorrected; writeErrors = $r.WriteErrorsUncorrected
  }
})
ConvertTo-Json -InputObject $out -Compress
"#;

fn restore_point(description: &str) -> io::Result<RestorePoint> {
    let clean: String = description
        .chars()
        .filter(|c| c.is_alphanumeric() || " :-_().".contains(*c))
        .take(64)
        .collect();
    crate::powershell::run_json_with(
        RESTORE_POINT,
        &[("SMD_DESC", &clean)],
        Some(Duration::from_secs(600)),
    )
}

/// SMART counters. Zero temperatures mean "not reported".
pub fn disk_reliability() -> io::Result<Vec<DiskReliability>> {
    let mut v: Vec<DiskReliability> =
        crate::powershell::run_json_with(DISK_RELIABILITY, &[], Some(Duration::from_secs(60)))?;
    for d in &mut v {
        d.temperature = d.temperature.filter(|t| *t > 0);
        d.temperature_max = d.temperature_max.filter(|t| *t > 0);
    }
    Ok(v)
}

fn scan_env_var(name: &str) -> Option<String> {
    std::env::var(name).ok()
}

fn admin_rules(ids: &[String]) -> Vec<smd_core::cleanup::Rule> {
    builtin_rules()
        .into_iter()
        .filter(|r| r.admin && ids.iter().any(|i| i == &r.id))
        .collect()
}

fn cleanup_scan(rule_ids: &[String], exclusions: &[String]) -> Vec<Category> {
    let running = crate::system::running_process_names();
    let var = scan_env_var;
    let env = ScanEnv {
        var: &var,
        exclusions,
        running: &running,
        now_ms: now_ms(),
    };
    admin_rules(rule_ids)
        .iter()
        .map(|r| scan_rule(r, &env))
        .collect()
}

fn cleanup_remove(rule_id: &str, paths: &[String], exclusions: &[String]) -> io::Result<Outcome> {
    let cats = cleanup_scan(&[rule_id.to_string()], exclusions);
    let cat = cats
        .into_iter()
        .next()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "not an admin cleanup rule"))?;
    let wanted: std::collections::HashSet<String> = paths
        .iter()
        .map(|p| smd_core::safety::normalize(p))
        .collect();
    let items: Vec<_> = cat
        .items
        .into_iter()
        .filter(|i| wanted.contains(&smd_core::safety::normalize(&i.path)))
        .collect();
    // System files always go to the Recycle Bin of the system drive (never to the quarantine
    // drive, where restoring them would need admin rights again).
    Ok(remove_items_to(&items, &Destination::RecycleBin).0)
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn sensor_exe() -> io::Result<PathBuf> {
    let me = std::env::current_exe()?;
    let dir = me
        .parent()
        .ok_or_else(|| io::Error::other("no program folder"))?;
    Ok(crate::hardware::helper_path(dir))
}

struct Session {
    sensors: Option<SensorHelper>,
}

fn json<T: Serialize>(v: io::Result<T>) -> Result<serde_json::Value, String> {
    v.map_err(|e| e.to_string())
        .and_then(|v| serde_json::to_value(v).map_err(|e| e.to_string()))
}

fn dispatch(s: &mut Session, op: Op) -> Result<serde_json::Value, String> {
    match op {
        Op::Ping | Op::Exit => Ok(serde_json::Value::Bool(true)),
        Op::RestorePoint { description } => json(restore_point(&description)),
        Op::EnableRestore => json(
            crate::powershell::run_text(ENABLE_RESTORE, &[], Some(Duration::from_secs(120)))
                .map(|_| true),
        ),
        Op::CleanupScan {
            rule_ids,
            exclusions,
        } => json(Ok(cleanup_scan(&rule_ids, &exclusions))),
        Op::CleanupRemove {
            rule_id,
            paths,
            exclusions,
        } => json(cleanup_remove(&rule_id, &paths, &exclusions)),
        Op::LeftoversRemove { app, paths } => json(
            crate::apps::admin_leftovers(&app, &paths)
                .map(|items| remove_items_to(&items, &Destination::RecycleBin).0),
        ),
        Op::StartupSet { id, enabled } => json(crate::startup::set_enabled_any(
            &id,
            enabled,
            now_ms(),
            true,
        )),
        Op::TweakApply { id } => {
            let t = smd_core::tweaks::find(&id).ok_or("unknown tweak")?;
            json(crate::tweaks::apply_machine(&t))
        }
        Op::TweakRevert { id, undo } => {
            let t = smd_core::tweaks::find(&id).ok_or("unknown tweak")?;
            if !smd_core::tweaks::undo_matches(&t, &undo) {
                return Err("the undo does not match this tweak".into());
            }
            json(crate::tweaks::revert_machine(&undo).map(|_| true))
        }
        Op::DiskReliability => json(disk_reliability()),
        Op::SensorsPoll => {
            if s.sensors.as_mut().is_none_or(|h| h.exited().is_some()) {
                s.sensors = Some(
                    SensorHelper::start(&sensor_exe().map_err(|e| e.to_string())?)
                        .map_err(|e| e.to_string())?,
                );
            }
            let latest: Option<SensorSnapshot> = s.sensors.as_ref().and_then(SensorHelper::latest);
            json(Ok(latest))
        }
        Op::SensorsStop => {
            s.sensors = None;
            Ok(serde_json::Value::Bool(true))
        }
    }
}

/// Runs the helper until the app disconnects. Returns the process exit code.
pub fn serve(pipe: &str, app_pid: u32) -> i32 {
    let file = match crate::elevate::connect_to_app(pipe, app_pid) {
        Ok(f) => f,
        Err(_) => return 2,
    };
    let last = Arc::new(AtomicI64::new(now_ms()));
    let watch = Arc::clone(&last);
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(Duration::from_secs(15));
            if now_ms() - watch.load(Ordering::Relaxed) > IDLE_EXIT.as_millis() as i64 {
                std::process::exit(0);
            }
        }
    });
    let Ok(read_half) = file.try_clone() else {
        return 3;
    };
    let mut writer = file;
    let mut session = Session { sensors: None };
    for line in BufReader::new(read_half).lines() {
        let Ok(line) = line else { break };
        last.store(now_ms(), Ordering::Relaxed);
        let Ok(req) = serde_json::from_str::<Request>(&line) else {
            break;
        };
        let exit = matches!(req.op, Op::Exit);
        let resp = match dispatch(&mut session, req.op) {
            Ok(value) => Response {
                id: req.id,
                value,
                error: None,
            },
            Err(e) => Response {
                id: req.id,
                value: serde_json::Value::Null,
                error: Some(e),
            },
        };
        last.store(now_ms(), Ordering::Relaxed);
        let Ok(mut out) = serde_json::to_vec(&resp) else {
            break;
        };
        out.push(b'\n');
        if writer.write_all(&out).is_err() || exit {
            break;
        }
    }
    0
}

/// `--helper <pipe> <pid>` from the command line, if present.
pub fn helper_args(args: &[String]) -> Option<(String, u32)> {
    let i = args.iter().position(|a| a == "--helper")?;
    let pipe = args.get(i + 1)?.clone();
    let pid = args.get(i + 2)?.parse().ok()?;
    Some((pipe, pid))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ops_round_trip_and_unknown_ops_are_refused() {
        let op = Op::CleanupRemove {
            rule_id: "system-temp".into(),
            paths: vec![r"C:\Windows\Temp\a".into()],
            exclusions: vec![],
        };
        let s = serde_json::to_string(&op).unwrap();
        assert!(s.contains(r#""kind":"cleanupRemove""#));
        assert!(serde_json::from_str::<Op>(&s).is_ok());
        assert!(serde_json::from_str::<Op>(r#"{"kind":"runCommand","cmd":"calc"}"#).is_err());
    }

    #[test]
    fn only_admin_rules_are_scanned() {
        let ids = vec!["user-temp".to_string(), "system-temp".to_string()];
        let rules = admin_rules(&ids);
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].id, "system-temp");
    }

    #[test]
    fn parses_helper_arguments() {
        let args: Vec<String> = ["smd.exe", "--helper", r"\\.\pipe\smd-helper-1-ab", "42"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert_eq!(
            helper_args(&args),
            Some((r"\\.\pipe\smd-helper-1-ab".into(), 42))
        );
        assert_eq!(helper_args(&args[..3]), None);
    }

    #[test]
    fn restore_point_json() {
        let r: RestorePoint = serde_json::from_str(r#"{"status":"recent","timeMs":5}"#).unwrap();
        assert!(matches!(r, RestorePoint::Recent { time_ms: Some(5) }));
        let r: RestorePoint = serde_json::from_str(r#"{"status":"off"}"#).unwrap();
        assert!(matches!(r, RestorePoint::Off));
    }
}
