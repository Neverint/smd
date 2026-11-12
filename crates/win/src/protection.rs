//! Protection: Microsoft Defender (status, threats, scan, signature update), firewall, and
//! app permissions for camera, microphone and location. No antivirus engine of our own.

use serde::{Deserialize, Serialize};
use std::io;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DefenderStatus {
    pub service_enabled: bool,
    pub antivirus_enabled: bool,
    pub real_time: bool,
    pub tamper_protected: Option<bool>,
    /// Unix ms.
    pub signatures_updated_ms: Option<i64>,
    pub signature_age_days: Option<u32>,
    pub quick_scan_end_ms: Option<i64>,
    pub full_scan_end_ms: Option<i64>,
    /// `Normal`, `Passive` (another antivirus is in charge), ...
    pub running_mode: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Threat {
    pub name: String,
    /// `Low`, `Moderate`, `High`, `Severe`, or empty.
    pub severity: String,
    pub detected_ms: Option<i64>,
    /// Defender's action status, e.g. `Quarantined`, `Removed`, `Allowed`, `Blocked`.
    pub status: String,
    /// Still needs attention (not removed, quarantined or allowed).
    pub active: bool,
    pub resources: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FirewallProfile {
    /// `Domain`, `Private`, `Public`
    pub name: String,
    pub enabled: bool,
}

const STATUS: &str = r#"
$ErrorActionPreference = 'Stop'
$s = Get-MpComputerStatus
function ms($d) { if ($d -and $d.Year -gt 1601) { [DateTimeOffset]::new($d.ToUniversalTime()).ToUnixTimeMilliseconds() } else { $null } }
ConvertTo-Json -Compress -InputObject ([pscustomobject]@{
  serviceEnabled = [bool]$s.AMServiceEnabled; antivirusEnabled = [bool]$s.AntivirusEnabled; realTime = [bool]$s.RealTimeProtectionEnabled
  tamperProtected = $s.IsTamperProtected; signaturesUpdatedMs = (ms $s.AntivirusSignatureLastUpdated); signatureAgeDays = [int]$s.AntivirusSignatureAge
  quickScanEndMs = (ms $s.QuickScanEndTime); fullScanEndMs = (ms $s.FullScanEndTime); runningMode = [string]$s.AMRunningMode })
"#;

const THREATS: &str = r#"
$ErrorActionPreference = 'Stop'
$names = @{}; $sev = @{}
foreach ($t in @(Get-MpThreat)) { $names[[string]$t.ThreatID] = [string]$t.ThreatName; $sev[[string]$t.ThreatID] = [int]$t.SeverityID }
$out = @(Get-MpThreatDetection | ForEach-Object {
  $id = [string]$_.ThreatID
  [pscustomobject]@{
    name = if ($names[$id]) { $names[$id] } else { "Threat $id" }
    severityId = $sev[$id]
    detectedMs = if ($_.InitialDetectionTime) { [DateTimeOffset]::new($_.InitialDetectionTime.ToUniversalTime()).ToUnixTimeMilliseconds() } else { $null }
    statusId = [int]$_.ThreatStatusID
    resources = @($_.Resources | ForEach-Object { [string]$_ })
  }
})
ConvertTo-Json -InputObject $out -Depth 4 -Compress
"#;

const FIREWALL: &str = r#"
$ErrorActionPreference = 'Stop'
ConvertTo-Json -Compress -InputObject @(Get-NetFirewallProfile | ForEach-Object { [pscustomobject]@{ name = [string]$_.Name; enabled = [bool]$_.Enabled } })
"#;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawThreat {
    name: String,
    severity_id: Option<i32>,
    detected_ms: Option<i64>,
    status_id: Option<i32>,
    resources: Option<Vec<String>>,
}

/// Defender `SeverityID` → name.
pub fn severity_name(id: i32) -> &'static str {
    match id {
        1 => "Low",
        2 => "Moderate",
        4 => "High",
        5 => "Severe",
        _ => "",
    }
}

/// Defender `ThreatStatusID` → (name, still active).
pub fn threat_status(id: i32) -> (&'static str, bool) {
    match id {
        1 => ("Detected", true),
        2 => ("Cleaned", false),
        3 => ("Quarantined", false),
        4 => ("Removed", false),
        5 => ("Allowed", false),
        6 => ("Blocked", false),
        102..=107 => ("ActionFailed", true),
        _ => ("Unknown", true),
    }
}

pub fn defender_status() -> io::Result<DefenderStatus> {
    crate::powershell::run_json_with(STATUS, &[], Some(Duration::from_secs(30)))
}

pub fn threats() -> io::Result<Vec<Threat>> {
    let raw: Vec<RawThreat> =
        crate::powershell::run_json_with(THREATS, &[], Some(Duration::from_secs(60)))?;
    let mut out: Vec<Threat> = raw
        .into_iter()
        .map(|r| {
            let (status, active) = threat_status(r.status_id.unwrap_or(0));
            Threat {
                name: r.name,
                severity: severity_name(r.severity_id.unwrap_or(0)).to_string(),
                detected_ms: r.detected_ms,
                status: status.to_string(),
                active,
                resources: r.resources.unwrap_or_default(),
            }
        })
        .collect();
    out.sort_by_key(|t| std::cmp::Reverse(t.detected_ms));
    Ok(out)
}

pub fn firewall() -> io::Result<Vec<FirewallProfile>> {
    crate::powershell::run_json_with(FIREWALL, &[], Some(Duration::from_secs(30)))
}

/// `%ProgramFiles%\Windows Defender\MpCmdRun.exe`, Defender's own command-line tool.
pub fn mpcmdrun() -> io::Result<PathBuf> {
    let base = std::env::var("ProgramW6432")
        .or_else(|_| std::env::var("ProgramFiles"))
        .unwrap_or_else(|_| r"C:\Program Files".into());
    let p = PathBuf::from(base).join(r"Windows Defender\MpCmdRun.exe");
    if p.is_file() {
        Ok(p)
    } else {
        Err(io::Error::new(
            io::ErrorKind::NotFound,
            "Microsoft Defender is not installed",
        ))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ScanKind {
    Quick,
    Full,
}

fn hidden(cmd: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
}

/// Starts a Defender scan. The caller waits on the child (and may kill it to cancel).
/// Exit code 0 = nothing found, 2 = threats found and handled by Defender.
pub fn start_scan(kind: ScanKind) -> io::Result<Child> {
    let mut cmd = Command::new(mpcmdrun()?);
    cmd.args([
        "-Scan",
        "-ScanType",
        if kind == ScanKind::Quick { "1" } else { "2" },
    ])
    .stdin(Stdio::null())
    .stdout(Stdio::null())
    .stderr(Stdio::null());
    hidden(&mut cmd);
    cmd.spawn()
}

/// Asks Defender to download the latest signatures (network: Microsoft Update).
pub fn update_signatures() -> io::Result<bool> {
    let mut cmd = Command::new(mpcmdrun()?);
    cmd.arg("-SignatureUpdate")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    hidden(&mut cmd);
    Ok(cmd.status()?.success())
}

/* ---------- App permissions (camera, microphone, location) ---------- */

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Capability {
    Webcam,
    Microphone,
    Location,
}

impl Capability {
    pub const ALL: [Capability; 3] = [
        Capability::Webcam,
        Capability::Microphone,
        Capability::Location,
    ];
    pub fn key(self) -> &'static str {
        match self {
            Capability::Webcam => "webcam",
            Capability::Microphone => "microphone",
            Capability::Location => "location",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionEntry {
    pub capability: Capability,
    /// Registry subkey name: a package family name, or a `#`-separated exe path for desktop apps.
    pub id: String,
    /// Readable name: the package name, or the exe path.
    pub app: String,
    /// Store/packaged app. Windows lets you allow or deny these one by one.
    pub packaged: bool,
    pub allowed: bool,
    pub last_used_ms: Option<i64>,
    /// Using it right now.
    pub in_use: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CapabilityState {
    pub capability: Capability,
    /// The "Let apps access ..." switch for your account.
    pub allowed: bool,
    pub apps: Vec<PermissionEntry>,
}

/// Readable app name from a consent-store key.
pub fn app_label(id: &str, packaged: bool) -> String {
    if packaged {
        // Microsoft.WindowsCamera_8wekyb3d8bbwe → Microsoft.WindowsCamera
        id.rsplit_once('_')
            .map(|(n, _)| n.to_string())
            .unwrap_or_else(|| id.to_string())
    } else {
        id.replace('#', "\\")
    }
}

/// FILETIME (100 ns since 1601) → Unix ms.
pub fn filetime_to_unix_ms(ft: u64) -> Option<i64> {
    const EPOCH_DIFF_100NS: u64 = 116_444_736_000_000_000;
    (ft > EPOCH_DIFF_100NS).then(|| ((ft - EPOCH_DIFF_100NS) / 10_000) as i64)
}

#[cfg(windows)]
mod perm {
    use super::*;
    use winreg::RegKey;
    use winreg::enums::{HKEY_CURRENT_USER, KEY_READ, KEY_SET_VALUE};

    const STORE: &str =
        r"Software\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\ConsentStore";

    fn entry(cap: Capability, key: &RegKey, id: String, packaged: bool) -> PermissionEntry {
        let value: String = key.get_value("Value").unwrap_or_default();
        let start: u64 = key.get_value("LastUsedTimeStart").unwrap_or(0);
        let stop: u64 = key.get_value("LastUsedTimeStop").unwrap_or(0);
        PermissionEntry {
            capability: cap,
            app: app_label(&id, packaged),
            id,
            packaged,
            allowed: !value.eq_ignore_ascii_case("Deny"),
            last_used_ms: filetime_to_unix_ms(stop.max(start)),
            in_use: start > 0 && stop == 0,
        }
    }

    pub fn list() -> Vec<CapabilityState> {
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let mut out = Vec::new();
        for cap in Capability::ALL {
            let Ok(root) = hkcu.open_subkey_with_flags(format!(r"{STORE}\{}", cap.key()), KEY_READ)
            else {
                continue;
            };
            let allowed = !root
                .get_value::<String, _>("Value")
                .unwrap_or_default()
                .eq_ignore_ascii_case("Deny");
            let mut apps = Vec::new();
            for name in root.enum_keys().flatten() {
                if name == "NonPackaged" {
                    if let Ok(np) = root.open_subkey_with_flags(&name, KEY_READ) {
                        for exe in np.enum_keys().flatten() {
                            if let Ok(k) = np.open_subkey_with_flags(&exe, KEY_READ) {
                                apps.push(entry(cap, &k, exe, false));
                            }
                        }
                    }
                } else if let Ok(k) = root.open_subkey_with_flags(&name, KEY_READ) {
                    apps.push(entry(cap, &k, name, true));
                }
            }
            apps.sort_by_key(|a| std::cmp::Reverse(a.last_used_ms));
            out.push(CapabilityState {
                capability: cap,
                allowed,
                apps,
            });
        }
        out
    }

    /// Allows or denies one packaged app. Returns the previous state.
    pub fn set(cap: Capability, id: &str, allow: bool) -> io::Result<bool> {
        if id.is_empty() || id.contains(['\\', '/', '#']) || id.contains("..") {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "only Store apps can be changed one by one",
            ));
        }
        let key = RegKey::predef(HKEY_CURRENT_USER).open_subkey_with_flags(
            format!(r"{STORE}\{}\{id}", cap.key()),
            KEY_READ | KEY_SET_VALUE,
        )?;
        let before = !key
            .get_value::<String, _>("Value")
            .unwrap_or_default()
            .eq_ignore_ascii_case("Deny");
        key.set_value("Value", &if allow { "Allow" } else { "Deny" })?;
        Ok(before)
    }
}

#[cfg(not(windows))]
mod perm {
    use super::*;
    pub fn list() -> Vec<CapabilityState> {
        Vec::new()
    }
    pub fn set(_cap: Capability, _id: &str, _allow: bool) -> io::Result<bool> {
        Err(io::Error::new(io::ErrorKind::Unsupported, "Windows only"))
    }
}

pub use perm::{list as permissions, set as set_permission};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_and_times() {
        assert_eq!(
            app_label("Microsoft.WindowsCamera_8wekyb3d8bbwe", true),
            "Microsoft.WindowsCamera"
        );
        assert_eq!(
            app_label("C:#Program Files#Zoom#bin#Zoom.exe", false),
            r"C:\Program Files\Zoom\bin\Zoom.exe"
        );
        assert_eq!(
            filetime_to_unix_ms(116_444_736_000_000_000 + 10_000),
            Some(1)
        );
        assert_eq!(filetime_to_unix_ms(0), None);
        assert_eq!(severity_name(5), "Severe");
        assert_eq!(threat_status(3), ("Quarantined", false));
        assert!(threat_status(1).1);
    }

    #[cfg(windows)]
    #[test]
    fn reads_live_protection_state() {
        let fw = firewall().unwrap();
        assert!(!fw.is_empty());
        let perms = permissions();
        assert!(
            perms
                .iter()
                .all(|c| c.apps.iter().all(|a| !a.id.is_empty()))
        );
        assert!(set_permission(Capability::Webcam, r"C:#evil", false).is_err());
        if std::env::var_os("CI").is_none() {
            let s = defender_status().unwrap();
            assert!(!s.running_mode.is_empty());
            assert!(threats().is_ok());
            assert!(mpcmdrun().is_ok());
        }
    }
}
