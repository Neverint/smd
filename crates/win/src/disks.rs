//! Physical disks and their health, from `Get-PhysicalDisk` (Storage module, no admin needed).
//! Full SMART attributes come in Phase 3 (Hardware Health).

use serde::{Deserialize, Serialize};
use smd_core::health::DiskHealth;
use std::io;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PhysicalDisk {
    pub name: String,
    /// `HDD`, `SSD` or `Unspecified`.
    pub media_type: String,
    pub bus_type: String,
    pub size_bytes: u64,
    pub health: DiskHealth,
    pub operational_status: String,
    /// Drive letters on this disk, like `["C:"]`.
    pub letters: Vec<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Raw {
    name: Option<String>,
    media_type: Option<String>,
    bus_type: Option<String>,
    size: Option<u64>,
    health: Option<String>,
    operational: Option<String>,
    letters: Option<String>,
}

const SCRIPT: &str = r#"
$ErrorActionPreference = 'Stop'
$disks = @(Get-PhysicalDisk | Sort-Object { [int]$_.DeviceId } | ForEach-Object {
  $n = [int]$_.DeviceId
  $letters = @(Get-Partition -DiskNumber $n -ErrorAction SilentlyContinue | Where-Object DriveLetter | ForEach-Object { "$($_.DriveLetter):" }) -join ','
  [pscustomobject]@{
    name        = [string]$_.FriendlyName
    mediaType   = [string]$_.MediaType
    busType     = [string]$_.BusType
    size        = [uint64]$_.Size
    health      = [string]$_.HealthStatus
    operational = [string]($_.OperationalStatus -join ', ')
    letters     = $letters
  }
})
ConvertTo-Json -InputObject $disks -Depth 4 -Compress
"#;

pub fn physical_disks() -> io::Result<Vec<PhysicalDisk>> {
    let raw: Vec<Raw> = crate::powershell::run_json(SCRIPT)?;
    Ok(raw
        .into_iter()
        .map(|r| PhysicalDisk {
            name: r.name.unwrap_or_default().trim().to_string(),
            media_type: r.media_type.unwrap_or_default(),
            bus_type: r.bus_type.unwrap_or_default(),
            size_bytes: r.size.unwrap_or(0),
            health: DiskHealth::from_windows(r.health.as_deref().unwrap_or("")),
            operational_status: r.operational.unwrap_or_default(),
            letters: r
                .letters
                .unwrap_or_default()
                .split(',')
                .filter(|s| !s.is_empty())
                .map(str::to_string)
                .collect(),
        })
        .collect())
}

#[cfg(all(test, windows))]
mod tests {
    #[test]
    fn lists_disks() {
        let disks = super::physical_disks().expect("Get-PhysicalDisk works without admin");
        assert!(!disks.is_empty());
        assert!(disks.iter().any(|d| d.size_bytes > 0));
    }
}
