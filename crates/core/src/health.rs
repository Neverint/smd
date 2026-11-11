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

#[cfg(test)]
mod tests {
    use super::*;

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
