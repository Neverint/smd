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

/// Maintenance from uptime: under 7 days good, 7-30 days warning, over 30 days critical.
/// Pending app and Windows updates are added in Phase 2.
pub fn maintenance_status(uptime_days: f64) -> Status {
    if uptime_days < 7.0 {
        Status::Good
    } else if uptime_days <= 30.0 {
        Status::Warning
    } else {
        Status::Critical
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
        let status = maintenance_status(days);
        areas.push(AreaResult {
            area: Area::Maintenance,
            status,
        });
        if status != Status::Good {
            recs.push((
                rank(status),
                Recommendation::Restart {
                    uptime_days: days.floor() as u32,
                },
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
        assert_eq!(maintenance_status(0.5), Status::Good);
        assert_eq!(maintenance_status(7.0), Status::Warning);
        assert_eq!(maintenance_status(30.0), Status::Warning);
        assert_eq!(maintenance_status(34.0), Status::Critical);
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
