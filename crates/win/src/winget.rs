//! App updates through the official `Microsoft.WinGet.Client` PowerShell module
//! (structured objects, no screen-scraping), and the pending Windows Update count.
//!
//! The module is not part of Windows: `install_module` downloads it from the PowerShell
//! Gallery for the current user, only when the user clicks "Set up" in the app.

use serde::{Deserialize, Serialize};
use std::io;
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppUpdate {
    pub id: String,
    pub name: String,
    pub installed: String,
    pub available: String,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateResult {
    /// `Ok`, `InstallError`, `NoApplicableUpgrade`...
    pub status: String,
    pub reboot_required: bool,
    pub installer_error_code: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowsUpdates {
    pub total: u32,
    pub security: u32,
    pub titles: Vec<String>,
}

const STATUS: &str = r#"ConvertTo-Json -InputObject ([bool](Get-Module -ListAvailable -Name Microsoft.WinGet.Client)) -Compress"#;

const INSTALL: &str = r#"
$ErrorActionPreference = 'Stop'
[Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12
if (-not (Get-PackageProvider -ListAvailable -Name NuGet -ErrorAction SilentlyContinue | Where-Object { $_.Version -ge [version]'2.8.5.201' })) {
  Install-PackageProvider -Name NuGet -MinimumVersion 2.8.5.201 -Scope CurrentUser -Force | Out-Null
}
Install-Module -Name Microsoft.WinGet.Client -Repository PSGallery -Scope CurrentUser -Force -AllowClobber
ConvertTo-Json -InputObject ([bool](Get-Module -ListAvailable -Name Microsoft.WinGet.Client)) -Compress
"#;

const LIST: &str = r#"
$ErrorActionPreference = 'Stop'
Import-Module Microsoft.WinGet.Client
$u = @(Get-WinGetPackage | Where-Object { $_.IsUpdateAvailable } | ForEach-Object {
  [pscustomobject]@{ id = [string]$_.Id; name = [string]$_.Name; installed = [string]$_.InstalledVersion; available = [string](@($_.AvailableVersions)[0]); source = [string]$_.Source }
})
ConvertTo-Json -InputObject $u -Depth 4 -Compress
"#;

const UPDATE: &str = r#"
$ErrorActionPreference = 'Stop'
Import-Module Microsoft.WinGet.Client
$r = Update-WinGetPackage -Id $env:SMD_ID -MatchOption Equals -Mode Silent
$code = $null; if ($r.InstallerErrorCode) { $code = [int64]$r.InstallerErrorCode }
ConvertTo-Json -InputObject ([pscustomobject]@{ status = [string]$r.Status; rebootRequired = [bool]$r.RebootRequired; installerErrorCode = $code }) -Compress
"#;

/// Uses the result of the last Windows Update check (offline search): fast, no download.
const WINDOWS_UPDATES: &str = r#"
$ErrorActionPreference = 'Stop'
$s = New-Object -ComObject Microsoft.Update.Session
$q = $s.CreateUpdateSearcher()
$q.Online = $false
$r = $q.Search("IsInstalled=0 and IsHidden=0 and Type='Software'")
$sec = 0; $titles = @()
foreach ($u in $r.Updates) {
  $titles += [string]$u.Title
  if ($u.MsrcSeverity -or @($u.Categories | Where-Object { $_.Name -eq 'Security Updates' }).Count -gt 0) { $sec++ }
}
ConvertTo-Json -InputObject ([pscustomobject]@{ total = [int]$r.Updates.Count; security = [int]$sec; titles = @($titles) }) -Depth 4 -Compress
"#;

pub fn module_installed() -> io::Result<bool> {
    crate::powershell::run_json_with(STATUS, &[], Some(Duration::from_secs(30)))
}

/// Downloads the WinGet module for the current user (network). Only on the user's request.
pub fn install_module() -> io::Result<bool> {
    crate::powershell::run_json_with(INSTALL, &[], Some(Duration::from_secs(300)))
}

pub fn list_updates(timeout: Duration) -> io::Result<Vec<AppUpdate>> {
    crate::powershell::run_json_with(LIST, &[], Some(timeout))
}

/// Updates one package silently with its own installer (which may ask for admin rights).
pub fn update(id: &str) -> io::Result<UpdateResult> {
    if !smd_core::apps::is_safe_package_id(id) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "unexpected package id",
        ));
    }
    crate::powershell::run_json_with(UPDATE, &[("SMD_ID", id)], Some(Duration::from_secs(1800)))
}

pub fn windows_updates(timeout: Duration) -> io::Result<WindowsUpdates> {
    crate::powershell::run_json_with(WINDOWS_UPDATES, &[], Some(timeout))
}

#[cfg(all(test, windows))]
mod tests {
    #[test]
    fn reads_status_and_windows_updates() {
        assert!(super::module_installed().is_ok());
        assert!(super::update("x; Remove-Item C:\\").is_err());
        if std::env::var_os("CI").is_none() {
            let wu = super::windows_updates(std::time::Duration::from_secs(60)).unwrap();
            assert!(wu.security <= wu.total);
        }
    }
}
