//! Startup items: the `StartupApproved` values Task Manager uses to turn items off and on.
//!
//! Each value is 12 bytes. The first byte is even when the item is on (`02`, `06`)
//! and odd when it is off (`03`, `07`); bytes 4-11 hold the time it was turned off.
//! A missing value means "on". The Run entry itself is never touched.

use serde::{Deserialize, Serialize};

pub fn is_enabled(approved: Option<&[u8]>) -> bool {
    match approved {
        Some(bytes) if !bytes.is_empty() => bytes[0] & 1 == 0,
        _ => true,
    }
}

/// The value to write. `filetime` is the current time as a Windows FILETIME (100 ns since 1601).
pub fn approved_value(enabled: bool, filetime: u64) -> [u8; 12] {
    let mut v = [0u8; 12];
    if enabled {
        v[0] = 0x02;
    } else {
        v[0] = 0x03;
        v[4..12].copy_from_slice(&filetime.to_le_bytes());
    }
    v
}

/// Converts Unix milliseconds to a Windows FILETIME.
pub fn filetime_from_unix_ms(ms: i64) -> u64 {
    const EPOCH_DIFF_100NS: u64 = 116_444_736_000_000_000;
    EPOCH_DIFF_100NS + (ms.max(0) as u64) * 10_000
}

/// Where a startup item is registered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Source {
    /// `HKCU\...\Run`
    UserRun,
    /// `HKLM\...\Run`
    MachineRun,
    /// `HKLM\SOFTWARE\WOW6432Node\...\Run`
    MachineRun32,
    /// Your Startup folder
    UserFolder,
    /// The Startup folder of all users
    CommonFolder,
}

impl Source {
    /// Machine-wide items need admin rights to change.
    pub fn needs_admin(self) -> bool {
        matches!(
            self,
            Source::MachineRun | Source::MachineRun32 | Source::CommonFolder
        )
    }
}

/// Startup load for Smart Care, from the number of third-party items that are on:
/// 6 or fewer good, 7-12 warning, more than 12 critical.
pub fn load_status(enabled_third_party: usize) -> crate::health::Status {
    use crate::health::Status;
    match enabled_third_party {
        0..=6 => Status::Good,
        7..=12 => Status::Warning,
        _ => Status::Critical,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::health::Status;

    #[test]
    fn reads_task_manager_values() {
        assert!(is_enabled(None));
        assert!(is_enabled(Some(&[])));
        assert!(is_enabled(Some(&[0x02, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0])));
        assert!(is_enabled(Some(&[0x06, 0, 0, 0])));
        assert!(!is_enabled(Some(&[0x03, 0, 0, 0, 1, 2, 3, 4, 5, 6, 7, 8])));
        assert!(!is_enabled(Some(&[0x07])));
    }

    #[test]
    fn writes_values_that_read_back() {
        let ft = filetime_from_unix_ms(1_700_000_000_000);
        let off = approved_value(false, ft);
        assert!(!is_enabled(Some(&off)));
        assert_eq!(u64::from_le_bytes(off[4..12].try_into().unwrap()), ft);
        let on = approved_value(true, ft);
        assert!(is_enabled(Some(&on)));
        assert_eq!(&on[1..], &[0u8; 11]);
    }

    #[test]
    fn filetime_epoch() {
        assert_eq!(filetime_from_unix_ms(0), 116_444_736_000_000_000);
    }

    #[test]
    fn load_thresholds_and_admin() {
        assert_eq!(load_status(6), Status::Good);
        assert_eq!(load_status(7), Status::Warning);
        assert_eq!(load_status(12), Status::Warning);
        assert_eq!(load_status(13), Status::Critical);
        assert!(Source::MachineRun.needs_admin());
        assert!(!Source::UserFolder.needs_admin());
    }
}
