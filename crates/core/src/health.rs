//! Smart Care health score.
//!
//! Start at 100, subtract 8 per warning and 18 per critical area, floor at 0.
//! Reclaimable space is shown as information and is never part of the score.

use serde::{Deserialize, Serialize};

/// The areas Smart Care rates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Area {
    FreeSpace,
    DiskHealth,
    Memory,
    Battery,
    Maintenance,
    StartupLoad,
    Protection,
    DriveLayout,
}

/// Traffic-light status of one area.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Status {
    Good,
    Warning,
    Critical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AreaResult {
    pub area: Area,
    pub status: Status,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Verdict {
    Excellent,
    Good,
    Fair,
    NeedsAttention,
}

pub const WARNING_PENALTY: u32 = 8;
pub const CRITICAL_PENALTY: u32 = 18;

/// Score out of 100 for a set of rated areas.
pub fn score(results: &[AreaResult]) -> u8 {
    let penalty: u32 = results
        .iter()
        .map(|r| match r.status {
            Status::Good => 0,
            Status::Warning => WARNING_PENALTY,
            Status::Critical => CRITICAL_PENALTY,
        })
        .sum();
    100u32.saturating_sub(penalty) as u8
}

/// 90+ Excellent, 75-89 Good, 60-74 Fair, under 60 Needs attention.
pub fn verdict(score: u8) -> Verdict {
    match score {
        90.. => Verdict::Excellent,
        75..=89 => Verdict::Good,
        60..=74 => Verdict::Fair,
        _ => Verdict::NeedsAttention,
    }
}

/// System drive free space: over 20 % good, 10-20 % warning, under 10 % critical.
pub fn free_space_status(free_percent: f64) -> Status {
    if free_percent > 20.0 {
        Status::Good
    } else if free_percent >= 10.0 {
        Status::Warning
    } else {
        Status::Critical
    }
}

/// Battery full-charge vs design capacity: 80 %+ good, 70-80 % warning, under 70 % critical.
pub fn battery_status(capacity_percent: f64) -> Status {
    if capacity_percent >= 80.0 {
        Status::Good
    } else if capacity_percent >= 70.0 {
        Status::Warning
    } else {
        Status::Critical
    }
}

/// Memory: sustained use (the lowest of several samples) over 80 % is a warning;
/// over 90 % with the commit charge above 90 % of its limit (heavy paging) is critical.
pub fn memory_status(min_load_percent: f64, commit_percent: f64) -> Status {
    if min_load_percent > 90.0 && commit_percent > 90.0 {
        Status::Critical
    } else if min_load_percent > 80.0 {
        Status::Warning
    } else {
        Status::Good
    }
}

/// Pending updates. `None` = not checked (the check is skipped, never guessed).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PendingUpdates {
    /// Apps with a newer version in winget.
    pub apps: Option<u32>,
    /// Windows updates waiting to install, including security ones.
    pub windows: Option<u32>,
    /// Of those, security updates.
    pub security: Option<u32>,
}

/// What Protection measured about Microsoft Defender.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ProtectionFacts {
    pub real_time: bool,
    /// Another antivirus is in charge (Defender in passive mode): Defender's own switches don't apply.
    pub other_antivirus: bool,
    pub signature_age_days: Option<u32>,
    pub active_threats: u32,
}

/// Defender on and signatures under 3 days: good. Stale signatures: warning.
/// Real-time protection off (with no other antivirus) or an active threat: critical.
pub fn protection_status(p: &ProtectionFacts) -> Status {
    if p.active_threats > 0 || (!p.real_time && !p.other_antivirus) {
        Status::Critical
    } else if !p.other_antivirus && p.signature_age_days.is_some_and(|d| d >= 3) {
        Status::Warning
    } else {
        Status::Good
    }
}

/// Uptime alone: under 7 days good, 7-30 days warning, over 30 days critical.
pub fn uptime_status(uptime_days: f64) -> Status {
    if uptime_days < 7.0 {
        Status::Good
    } else if uptime_days <= 30.0 {
        Status::Warning
    } else {
        Status::Critical
    }
}

/// Maintenance: uptime, plus pending updates. Any app or Windows update is a warning;
/// a pending security update is critical.
pub fn maintenance_status(uptime_days: f64, pending: &PendingUpdates) -> Status {
    let uptime = uptime_status(uptime_days);
    if uptime == Status::Critical || pending.security.unwrap_or(0) > 0 {
        Status::Critical
    } else if uptime == Status::Warning
        || pending.apps.unwrap_or(0) > 0
        || pending.windows.unwrap_or(0) > 0
    {
        Status::Warning
    } else {
        Status::Good
    }
}

/// Disk health as Windows reports it (`HealthStatus` of `Get-PhysicalDisk`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DiskHealth {
    Healthy,
    Warning,
    Unhealthy,
    Unknown,
}

impl DiskHealth {
    pub fn from_windows(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "healthy" | "0" => DiskHealth::Healthy,
            "warning" | "1" => DiskHealth::Warning,
            "unhealthy" | "2" => DiskHealth::Unhealthy,
            _ => DiskHealth::Unknown,
        }
    }
}

/// Any disk in Warning or Unhealthy state is critical. Unknown (some USB enclosures) is not penalized.
pub fn disk_health_status(disks: &[DiskHealth]) -> Status {
    if disks
        .iter()
        .any(|d| matches!(d, DiskHealth::Warning | DiskHealth::Unhealthy))
    {
        Status::Critical
    } else {
        Status::Good
    }
}

/// What Smart Care measured. Fields are `None` when the check could not run.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Facts {
    pub system_drive: String,
    pub system_free_percent: Option<f64>,
    pub uptime_days: Option<f64>,
    pub memory_min_load_percent: Option<f64>,
    pub memory_commit_percent: Option<f64>,
    /// Name and health of each physical disk.
    pub disks: Option<Vec<(String, DiskHealth)>>,
    /// Space the Cleanup scan found (information only, not part of the score).
    pub reclaimable_bytes: u64,
    pub updates: PendingUpdates,
    /// Third-party startup items that are on.
    pub startup_enabled_third_party: Option<usize>,
    pub protection: Option<ProtectionFacts>,
    /// Lowest battery health (full charge vs design), laptops only.
    pub battery_health_percent: Option<f64>,
    /// Drive Organizer: layout status and bytes in the wrong place.
    pub drive_layout: Option<(Status, u64)>,
}

/// One suggestion, ranked by impact. The UI words it from `kind`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Recommendation {
    ReplaceDisk { disk: String },
    FreeSpace { drive: String, free_percent: f64 },
    Restart { uptime_days: u32 },
    CloseApps { load_percent: u32 },
    CleanJunk { bytes: u64 },
    InstallSecurityUpdates { count: u32 },
    InstallWindowsUpdates { count: u32 },
    UpdateApps { count: u32 },
    ReviewStartup { count: u32 },
    ResolveThreats { count: u32 },
    TurnOnRealTimeProtection,
    UpdateSignatures { days: u32 },
    CheckBattery { health_percent: f64 },
    ReviewDriveLayout { bytes: u64 },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Assessment {
    pub score: u8,
    pub verdict: Verdict,
    pub areas: Vec<AreaResult>,
    pub recommendations: Vec<Recommendation>,
}

/// Junk below this size is not worth a recommendation.
pub const JUNK_RECOMMENDATION_MIN: u64 = 500 * 1024 * 1024;
pub const MAX_RECOMMENDATIONS: usize = 5;

/// Rates every measured area, scores them and ranks up to 5 recommendations.
pub fn assess(f: &Facts) -> Assessment {
    let mut areas = Vec::new();
    // (severity rank, recommendation): critical first, then warnings, then information.
    let mut recs: Vec<(u8, Recommendation)> = Vec::new();
    let rank = |s: Status| match s {
        Status::Critical => 0,
        Status::Warning => 1,
        Status::Good => 2,
    };

    if let Some(disks) = &f.disks {
        let status = disk_health_status(&disks.iter().map(|d| d.1).collect::<Vec<_>>());
        areas.push(AreaResult {
            area: Area::DiskHealth,
            status,
        });
        for (name, health) in disks {
            if matches!(health, DiskHealth::Warning | DiskHealth::Unhealthy) {
                recs.push((0, Recommendation::ReplaceDisk { disk: name.clone() }));
            }
        }
    }
    if let Some(free) = f.system_free_percent {
        let status = free_space_status(free);
        areas.push(AreaResult {
            area: Area::FreeSpace,
            status,
        });
        if status != Status::Good {
            recs.push((
                rank(status),
                Recommendation::FreeSpace {
                    drive: f.system_drive.clone(),
                    free_percent: free,
                },
            ));
        }
    }
    if let Some(load) = f.memory_min_load_percent {
        let status = memory_status(load, f.memory_commit_percent.unwrap_or(0.0));
        areas.push(AreaResult {
            area: Area::Memory,
            status,
        });
        if status != Status::Good {
            recs.push((
                rank(status),
                Recommendation::CloseApps {
                    load_percent: load.round() as u32,
                },
            ));
        }
    }
    if let Some(days) = f.uptime_days {
        let status = maintenance_status(days, &f.updates);
        areas.push(AreaResult {
            area: Area::Maintenance,
            status,
        });
        let uptime = uptime_status(days);
        if uptime != Status::Good {
            recs.push((
                rank(uptime),
                Recommendation::Restart {
                    uptime_days: days.floor() as u32,
                },
            ));
        }
        let security = f.updates.security.unwrap_or(0);
        let other_windows = f.updates.windows.unwrap_or(0).saturating_sub(security);
        if security > 0 {
            recs.push((
                0,
                Recommendation::InstallSecurityUpdates { count: security },
            ));
        }
        if other_windows > 0 {
            recs.push((
                1,
                Recommendation::InstallWindowsUpdates {
                    count: other_windows,
                },
            ));
        }
        if let Some(apps) = f.updates.apps.filter(|n| *n > 0) {
            recs.push((1, Recommendation::UpdateApps { count: apps }));
        }
    }
    if let Some(p) = &f.protection {
        let status = protection_status(p);
        areas.push(AreaResult {
            area: Area::Protection,
            status,
        });
        if p.active_threats > 0 {
            recs.push((
                0,
                Recommendation::ResolveThreats {
                    count: p.active_threats,
                },
            ));
        }
        if !p.real_time && !p.other_antivirus {
            recs.push((0, Recommendation::TurnOnRealTimeProtection));
        }
        if let Some(days) = p
            .signature_age_days
            .filter(|d| *d >= 3 && !p.other_antivirus)
        {
            recs.push((1, Recommendation::UpdateSignatures { days }));
        }
    }
    if let Some(health) = f.battery_health_percent {
        let status = battery_status(health);
        areas.push(AreaResult {
            area: Area::Battery,
            status,
        });
        if status != Status::Good {
            recs.push((
                rank(status),
                Recommendation::CheckBattery {
                    health_percent: health,
                },
            ));
        }
    }
    if let Some((status, bytes)) = f.drive_layout {
        areas.push(AreaResult {
            area: Area::DriveLayout,
            status,
        });
        if status != Status::Good {
            recs.push((rank(status), Recommendation::ReviewDriveLayout { bytes }));
        }
    }
    if let Some(n) = f.startup_enabled_third_party {
        let status = crate::startup::load_status(n);
        areas.push(AreaResult {
            area: Area::StartupLoad,
            status,
        });
        if status != Status::Good {
            recs.push((
                rank(status),
                Recommendation::ReviewStartup { count: n as u32 },
            ));
        }
    }
    if f.reclaimable_bytes >= JUNK_RECOMMENDATION_MIN {
        // Low space makes cleaning more urgent.
        let urgent = f.system_free_percent.is_some_and(|p| p <= 20.0);
        recs.push((
            if urgent { 1 } else { 2 },
            Recommendation::CleanJunk {
                bytes: f.reclaimable_bytes,
            },
        ));
    }

    recs.sort_by_key(|(r, _)| *r);
    let recommendations = recs
        .into_iter()
        .take(MAX_RECOMMENDATIONS)
        .map(|(_, r)| r)
        .collect();
    let score = score(&areas);
    Assessment {
        score,
        verdict: verdict(score),
        areas,
        recommendations,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_thresholds() {
        assert_eq!(memory_status(60.0, 50.0), Status::Good);
        assert_eq!(memory_status(80.0, 95.0), Status::Good);
        assert_eq!(memory_status(85.0, 70.0), Status::Warning);
        assert_eq!(memory_status(95.0, 80.0), Status::Warning);
        assert_eq!(memory_status(95.0, 95.0), Status::Critical);
    }

    #[test]
    fn maintenance_thresholds() {
        let none = PendingUpdates::default();
        assert_eq!(maintenance_status(0.5, &none), Status::Good);
        assert_eq!(maintenance_status(7.0, &none), Status::Warning);
        assert_eq!(maintenance_status(30.0, &none), Status::Warning);
        assert_eq!(maintenance_status(34.0, &none), Status::Critical);
        let apps = PendingUpdates {
            apps: Some(3),
            ..none
        };
        assert_eq!(maintenance_status(1.0, &apps), Status::Warning);
        let security = PendingUpdates {
            windows: Some(2),
            security: Some(1),
            ..none
        };
        assert_eq!(maintenance_status(1.0, &security), Status::Critical);
        let zero = PendingUpdates {
            apps: Some(0),
            windows: Some(0),
            security: Some(0),
        };
        assert_eq!(maintenance_status(1.0, &zero), Status::Good);
    }

    #[test]
    fn protection_and_battery() {
        let ok = ProtectionFacts {
            real_time: true,
            other_antivirus: false,
            signature_age_days: Some(0),
            active_threats: 0,
        };
        assert_eq!(protection_status(&ok), Status::Good);
        assert_eq!(
            protection_status(&ProtectionFacts {
                signature_age_days: Some(4),
                ..ok
            }),
            Status::Warning
        );
        assert_eq!(
            protection_status(&ProtectionFacts {
                real_time: false,
                ..ok
            }),
            Status::Critical
        );
        assert_eq!(
            protection_status(&ProtectionFacts {
                active_threats: 1,
                ..ok
            }),
            Status::Critical
        );
        let passive = ProtectionFacts {
            real_time: false,
            other_antivirus: true,
            signature_age_days: Some(30),
            active_threats: 0,
        };
        assert_eq!(
            protection_status(&passive),
            Status::Good,
            "another antivirus is in charge"
        );

        let f = Facts {
            protection: Some(ProtectionFacts {
                real_time: false,
                other_antivirus: false,
                signature_age_days: Some(5),
                active_threats: 2,
            }),
            battery_health_percent: Some(72.0),
            ..Facts::default()
        };
        let a = assess(&f);
        assert_eq!(a.score, 100 - 18 - 8);
        assert_eq!(
            a.recommendations,
            vec![
                Recommendation::ResolveThreats { count: 2 },
                Recommendation::TurnOnRealTimeProtection,
                Recommendation::UpdateSignatures { days: 5 },
                Recommendation::CheckBattery {
                    health_percent: 72.0
                },
            ]
        );
        let json = serde_json::to_string(&Recommendation::TurnOnRealTimeProtection).unwrap();
        assert_eq!(json, r#"{"kind":"turnOnRealTimeProtection"}"#);
    }

    #[test]
    fn updates_and_startup_recommendations() {
        let f = Facts {
            uptime_days: Some(2.0),
            updates: PendingUpdates {
                apps: Some(4),
                windows: Some(3),
                security: Some(1),
            },
            startup_enabled_third_party: Some(9),
            ..Facts::default()
        };
        let a = assess(&f);
        // maintenance critical (security) 18, startup warning 8
        assert_eq!(a.score, 100 - 18 - 8);
        assert_eq!(
            a.recommendations,
            vec![
                Recommendation::InstallSecurityUpdates { count: 1 },
                Recommendation::InstallWindowsUpdates { count: 2 },
                Recommendation::UpdateApps { count: 4 },
                Recommendation::ReviewStartup { count: 9 },
            ]
        );
    }

    #[test]
    fn disk_health_parsing() {
        assert_eq!(DiskHealth::from_windows("Healthy"), DiskHealth::Healthy);
        assert_eq!(
            DiskHealth::from_windows(" unhealthy "),
            DiskHealth::Unhealthy
        );
        assert_eq!(DiskHealth::from_windows("1"), DiskHealth::Warning);
        assert_eq!(DiskHealth::from_windows("whatever"), DiskHealth::Unknown);
        assert_eq!(
            disk_health_status(&[DiskHealth::Healthy, DiskHealth::Unknown]),
            Status::Good
        );
        assert_eq!(
            disk_health_status(&[DiskHealth::Healthy, DiskHealth::Warning]),
            Status::Critical
        );
    }

    #[test]
    fn assess_healthy_pc() {
        let f = Facts {
            system_drive: "C:".into(),
            system_free_percent: Some(40.0),
            uptime_days: Some(1.2),
            memory_min_load_percent: Some(45.0),
            memory_commit_percent: Some(50.0),
            disks: Some(vec![("SSD".into(), DiskHealth::Healthy)]),
            reclaimable_bytes: 100,
            ..Facts::default()
        };
        let a = assess(&f);
        assert_eq!(a.score, 100);
        assert_eq!(a.verdict, Verdict::Excellent);
        assert_eq!(a.areas.len(), 4);
        assert!(a.recommendations.is_empty());
    }

    #[test]
    fn assess_ranks_recommendations() {
        let f = Facts {
            system_drive: "C:".into(),
            system_free_percent: Some(18.0),
            uptime_days: Some(34.5),
            memory_min_load_percent: Some(50.0),
            memory_commit_percent: None,
            disks: Some(vec![("Old HDD".into(), DiskHealth::Unhealthy)]),
            reclaimable_bytes: 12 * 1024 * 1024 * 1024,
            ..Facts::default()
        };
        let a = assess(&f);
        // disk critical 18, uptime critical 18, free space warning 8
        assert_eq!(a.score, 100 - 18 - 18 - 8);
        assert_eq!(
            a.recommendations,
            vec![
                Recommendation::ReplaceDisk {
                    disk: "Old HDD".into()
                },
                Recommendation::Restart { uptime_days: 34 },
                Recommendation::FreeSpace {
                    drive: "C:".into(),
                    free_percent: 18.0
                },
                Recommendation::CleanJunk {
                    bytes: 12 * 1024 * 1024 * 1024
                },
            ]
        );
    }

    #[test]
    fn unmeasured_areas_are_not_scored() {
        let a = assess(&Facts::default());
        assert_eq!(a.score, 100);
        assert!(a.areas.is_empty());
    }

    #[test]
    fn recommendation_json_shape() {
        let json = serde_json::to_string(&Recommendation::FreeSpace {
            drive: "C:".into(),
            free_percent: 12.5,
        })
        .unwrap();
        assert_eq!(
            json,
            r#"{"kind":"freeSpace","drive":"C:","freePercent":12.5}"#
        );
    }

    fn r(area: Area, status: Status) -> AreaResult {
        AreaResult { area, status }
    }

    #[test]
    fn all_good_is_100() {
        let results = [
            r(Area::FreeSpace, Status::Good),
            r(Area::Memory, Status::Good),
        ];
        assert_eq!(score(&results), 100);
        assert_eq!(score(&[]), 100);
    }

    #[test]
    fn penalties_add_up() {
        let results = [
            r(Area::FreeSpace, Status::Warning),
            r(Area::Protection, Status::Critical),
            r(Area::Maintenance, Status::Warning),
        ];
        assert_eq!(score(&results), 100 - 8 - 18 - 8);
    }

    #[test]
    fn floors_at_zero() {
        let results = [r(Area::DiskHealth, Status::Critical); 8];
        assert_eq!(score(&results), 0);
    }

    #[test]
    fn verdict_bands() {
        assert_eq!(verdict(100), Verdict::Excellent);
        assert_eq!(verdict(90), Verdict::Excellent);
        assert_eq!(verdict(89), Verdict::Good);
        assert_eq!(verdict(75), Verdict::Good);
        assert_eq!(verdict(74), Verdict::Fair);
        assert_eq!(verdict(60), Verdict::Fair);
        assert_eq!(verdict(59), Verdict::NeedsAttention);
        assert_eq!(verdict(0), Verdict::NeedsAttention);
    }

    #[test]
    fn free_space_thresholds() {
        assert_eq!(free_space_status(21.3), Status::Good);
        assert_eq!(free_space_status(20.0), Status::Warning);
        assert_eq!(free_space_status(10.0), Status::Warning);
        assert_eq!(free_space_status(9.9), Status::Critical);
    }

    #[test]
    fn battery_thresholds() {
        assert_eq!(battery_status(95.0), Status::Good);
        assert_eq!(battery_status(80.0), Status::Good);
        assert_eq!(battery_status(75.0), Status::Warning);
        assert_eq!(battery_status(69.9), Status::Critical);
    }

    #[test]
    fn serializes_camel_case() {
        let json = serde_json::to_string(&r(Area::DriveLayout, Status::Critical)).unwrap();
        assert_eq!(json, r#"{"area":"driveLayout","status":"critical"}"#);
        let v = serde_json::to_string(&Verdict::NeedsAttention).unwrap();
        assert_eq!(v, r#""needsAttention""#);
    }
}
