//! Read-only lists: non-Microsoft scheduled tasks and services, and the last boot.

use serde::{Deserialize, Serialize};
use std::io;
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScheduledTask {
    pub name: String,
    pub path: String,
    /// `Ready`, `Disabled`, `Running`...
    pub state: String,
    pub author: String,
    pub program: Option<String>,
    pub publisher: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Service {
    pub name: String,
    pub display_name: String,
    /// `Running`, `Stopped`...
    pub state: String,
    /// `Auto`, `Manual`, `Disabled`...
    pub start_mode: String,
    pub program: Option<String>,
    pub publisher: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BootInfo {
    /// Unix ms of the last start.
    pub last_boot_ms: i64,
    /// Boot duration Windows recorded (Diagnostics-Performance event 100), if any.
    pub boot_duration_ms: Option<u64>,
    /// Main path duration (time to a usable desktop), if recorded.
    pub main_path_ms: Option<u64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawTask {
    name: Option<String>,
    path: Option<String>,
    state: Option<String>,
    author: Option<String>,
    exec: Option<String>,
}

const TASKS: &str = r#"
$ErrorActionPreference = 'Stop'
$t = @(Get-ScheduledTask | Where-Object { $_.TaskPath -notlike '\Microsoft\*' } | ForEach-Object {
  $a = @($_.Actions | Where-Object { $_.Execute }) | Select-Object -First 1
  [pscustomobject]@{ name = [string]$_.TaskName; path = [string]$_.TaskPath; state = [string]$_.State; author = [string]$_.Author; exec = if ($a) { [string]$a.Execute } else { '' } }
})
ConvertTo-Json -InputObject $t -Depth 4 -Compress
"#;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawService {
    name: Option<String>,
    display: Option<String>,
    state: Option<String>,
    start: Option<String>,
    path: Option<String>,
}

const SERVICES: &str = r#"
$ErrorActionPreference = 'Stop'
$s = @(Get-CimInstance Win32_Service | ForEach-Object {
  [pscustomobject]@{ name = [string]$_.Name; display = [string]$_.DisplayName; state = [string]$_.State; start = [string]$_.StartMode; path = [string]$_.PathName }
})
ConvertTo-Json -InputObject $s -Depth 4 -Compress
"#;

const BOOT: &str = r#"
$os = Get-CimInstance Win32_OperatingSystem
$boot = [DateTimeOffset]::new($os.LastBootUpTime).ToUnixTimeMilliseconds()
$d = $null; $m = $null
try {
  $e = Get-WinEvent -FilterHashtable @{ LogName = 'Microsoft-Windows-Diagnostics-Performance/Operational'; Id = 100 } -MaxEvents 1 -ErrorAction Stop
  $x = ([xml]$e.ToXml()).Event.EventData.Data
  $d = [uint64](($x | Where-Object Name -eq 'BootTime').'#text')
  $m = [uint64](($x | Where-Object Name -eq 'MainPathBootTime').'#text')
} catch {}
ConvertTo-Json -InputObject ([pscustomobject]@{ lastBootMs = $boot; bootDurationMs = $d; mainPathMs = $m }) -Compress
"#;

fn publisher(program: &Option<String>) -> Option<String> {
    program.as_deref().and_then(crate::shell::file_company)
}

fn is_microsoft(publisher: &Option<String>) -> bool {
    publisher
        .as_deref()
        .is_some_and(smd_core::apps::is_microsoft)
}

/// Scheduled tasks outside `\Microsoft\`, minus those whose program is signed by Microsoft.
pub fn scheduled_tasks() -> io::Result<Vec<ScheduledTask>> {
    let raw: Vec<RawTask> =
        crate::powershell::run_json_with(TASKS, &[], Some(Duration::from_secs(60)))?;
    let mut out: Vec<ScheduledTask> = raw
        .into_iter()
        .map(|r| {
            let program = r
                .exec
                .as_deref()
                .filter(|e| !e.is_empty())
                .and_then(crate::shell::program_of);
            let publisher = publisher(&program);
            ScheduledTask {
                name: r.name.unwrap_or_default(),
                path: r.path.unwrap_or_default(),
                state: r.state.unwrap_or_default(),
                author: r.author.unwrap_or_default(),
                program,
                publisher,
            }
        })
        .filter(|t| !is_microsoft(&t.publisher))
        .collect();
    out.sort_by_key(|a| a.name.to_lowercase());
    Ok(out)
}

/// Services whose program is not published by Microsoft (svchost-hosted services are Microsoft's).
pub fn third_party_services() -> io::Result<Vec<Service>> {
    let raw: Vec<RawService> =
        crate::powershell::run_json_with(SERVICES, &[], Some(Duration::from_secs(60)))?;
    let mut out: Vec<Service> = raw
        .into_iter()
        .filter_map(|r| {
            let program = r.path.as_deref().and_then(crate::shell::program_of);
            let publisher = publisher(&program);
            if is_microsoft(&publisher)
                || program
                    .as_deref()
                    .is_some_and(|p| p.to_lowercase().ends_with(r"\svchost.exe"))
            {
                return None;
            }
            Some(Service {
                name: r.name.unwrap_or_default(),
                display_name: r.display.unwrap_or_default(),
                state: r.state.unwrap_or_default(),
                start_mode: r.start.unwrap_or_default(),
                program,
                publisher,
            })
        })
        .collect();
    out.sort_by_key(|a| a.display_name.to_lowercase());
    Ok(out)
}

pub fn boot_info() -> io::Result<BootInfo> {
    crate::powershell::run_json_with(BOOT, &[], Some(Duration::from_secs(30)))
}

impl<'de> Deserialize<'de> for BootInfo {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Raw {
            last_boot_ms: i64,
            boot_duration_ms: Option<u64>,
            main_path_ms: Option<u64>,
        }
        let r = Raw::deserialize(d)?;
        Ok(BootInfo {
            last_boot_ms: r.last_boot_ms,
            boot_duration_ms: r.boot_duration_ms.filter(|v| *v > 0),
            main_path_ms: r.main_path_ms.filter(|v| *v > 0),
        })
    }
}

#[cfg(all(test, windows))]
mod tests {
    #[test]
    fn reads_background_lists() {
        let tasks = super::scheduled_tasks().unwrap();
        assert!(tasks.iter().all(|t| !t.path.starts_with(r"\Microsoft\")));
        let services = super::third_party_services().unwrap();
        assert!(services.iter().all(|s| !s.name.is_empty()));
        let boot = super::boot_info().unwrap();
        assert!(boot.last_boot_ms > 1_500_000_000_000);
    }
}
