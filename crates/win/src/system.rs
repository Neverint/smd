//! Read-only system facts: drives, memory, uptime, running programs, Recycle Bin size.

use serde::Serialize;
use std::collections::HashSet;
use std::io;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum DriveKind {
    Fixed,
    Removable,
    Network,
    Optical,
    Ram,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DriveInfo {
    /// `C:`
    pub letter: String,
    /// `C:\`
    pub root: String,
    pub label: String,
    pub file_system: String,
    pub kind: DriveKind,
    pub total_bytes: u64,
    pub free_bytes: u64,
    pub is_system: bool,
}

impl DriveInfo {
    pub fn free_percent(&self) -> f64 {
        if self.total_bytes == 0 {
            0.0
        } else {
            self.free_bytes as f64 * 100.0 / self.total_bytes as f64
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryInfo {
    /// Percentage of physical memory in use (0-100), as Task Manager shows it.
    pub load_percent: u32,
    pub total_bytes: u64,
    pub available_bytes: u64,
    /// Committed memory vs the commit limit (RAM + page file), in percent.
    pub commit_percent: f64,
}

/// `C:` (from `%SystemDrive%`).
pub fn system_drive() -> String {
    std::env::var("SystemDrive").unwrap_or_else(|_| "C:".into())
}

#[cfg(windows)]
mod imp {
    use super::*;
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::Storage::FileSystem::{
        GetDiskFreeSpaceExW, GetDriveTypeW, GetLogicalDrives, GetVolumeInformationW,
    };
    use windows::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW,
        TH32CS_SNAPPROCESS,
    };
    use windows::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};
    use windows::Win32::System::SystemInformation::{
        GetTickCount64, GlobalMemoryStatusEx, MEMORYSTATUSEX,
    };
    use windows::Win32::UI::Shell::{
        SHERB_NOCONFIRMATION, SHERB_NOPROGRESSUI, SHERB_NOSOUND, SHEmptyRecycleBinW, SHQUERYRBINFO,
        SHQueryRecycleBinW,
    };
    use windows::core::{HSTRING, PCWSTR};

    fn wide_to_string(buf: &[u16]) -> String {
        let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
        String::from_utf16_lossy(&buf[..end])
    }

    pub fn drives() -> Vec<DriveInfo> {
        let mask = unsafe { GetLogicalDrives() };
        let system = system_drive().to_uppercase();
        let mut out = Vec::new();
        for i in 0..26u32 {
            if mask & (1 << i) == 0 {
                continue;
            }
            let letter = format!("{}:", (b'A' + i as u8) as char);
            let root = format!("{letter}\\");
            let root_w = HSTRING::from(root.as_str());
            let kind = match unsafe { GetDriveTypeW(&root_w) } {
                2 => DriveKind::Removable,
                3 => DriveKind::Fixed,
                4 => DriveKind::Network,
                5 => DriveKind::Optical,
                6 => DriveKind::Ram,
                _ => DriveKind::Unknown,
            };
            let mut label = [0u16; 261];
            let mut fs = [0u16; 261];
            let has_volume = unsafe {
                GetVolumeInformationW(&root_w, Some(&mut label), None, None, None, Some(&mut fs))
            }
            .is_ok();
            if !has_volume {
                // Empty card reader or optical drive: skip.
                continue;
            }
            let (mut free, mut total) = (0u64, 0u64);
            if unsafe { GetDiskFreeSpaceExW(&root_w, Some(&mut free), Some(&mut total), None) }
                .is_err()
            {
                continue;
            }
            out.push(DriveInfo {
                is_system: letter == system,
                letter,
                root,
                label: wide_to_string(&label),
                file_system: wide_to_string(&fs),
                kind,
                total_bytes: total,
                free_bytes: free,
            });
        }
        out
    }

    pub fn memory() -> io::Result<MemoryInfo> {
        let mut m = MEMORYSTATUSEX {
            dwLength: std::mem::size_of::<MEMORYSTATUSEX>() as u32,
            ..Default::default()
        };
        unsafe { GlobalMemoryStatusEx(&mut m) }.map_err(io::Error::other)?;
        let commit_limit = m.ullTotalPageFile;
        let commit_used = commit_limit.saturating_sub(m.ullAvailPageFile);
        Ok(MemoryInfo {
            load_percent: m.dwMemoryLoad,
            total_bytes: m.ullTotalPhys,
            available_bytes: m.ullAvailPhys,
            commit_percent: if commit_limit == 0 {
                0.0
            } else {
                commit_used as f64 * 100.0 / commit_limit as f64
            },
        })
    }

    pub fn uptime_seconds() -> u64 {
        let ms = unsafe { GetTickCount64() };
        ms / 1000
    }

    pub fn running_process_names() -> HashSet<String> {
        let mut names = HashSet::new();
        let Ok(snap) = (unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) }) else {
            return names;
        };
        let mut entry = PROCESSENTRY32W {
            dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };
        let mut ok = unsafe { Process32FirstW(snap, &mut entry) }.is_ok();
        while ok {
            names.insert(wide_to_string(&entry.szExeFile).to_lowercase());
            ok = unsafe { Process32NextW(snap, &mut entry) }.is_ok();
        }
        let _ = unsafe { CloseHandle(snap) };
        names
    }

    /// Size and item count of the Recycle Bin on every drive.
    pub fn recycle_bin_size() -> io::Result<(u64, u64)> {
        let mut info = SHQUERYRBINFO {
            cbSize: std::mem::size_of::<SHQUERYRBINFO>() as u32,
            ..Default::default()
        };
        unsafe { SHQueryRecycleBinW(PCWSTR::null(), &mut info) }.map_err(io::Error::other)?;
        Ok((info.i64Size.max(0) as u64, info.i64NumItems.max(0) as u64))
    }

    /// Permanently empties the Recycle Bin on every drive. Cannot be undone.
    pub fn empty_recycle_bin() -> io::Result<()> {
        let (_, items) = recycle_bin_size()?;
        if items == 0 {
            return Ok(());
        }
        unsafe {
            SHEmptyRecycleBinW(
                None,
                PCWSTR::null(),
                SHERB_NOCONFIRMATION | SHERB_NOPROGRESSUI | SHERB_NOSOUND,
            )
        }
        .map_err(io::Error::other)
    }

    /// Running on battery right now (a laptop unplugged).
    pub fn on_battery() -> bool {
        let mut s = SYSTEM_POWER_STATUS::default();
        unsafe { GetSystemPowerStatus(&mut s) }.is_ok() && s.ACLineStatus == 0
    }

    /// True when Windows reports a system battery (laptops, tablets).
    pub fn has_battery() -> bool {
        let mut s = SYSTEM_POWER_STATUS::default();
        if unsafe { GetSystemPowerStatus(&mut s) }.is_err() {
            return false;
        }
        // 128 = no system battery, 255 = unknown.
        s.BatteryFlag != 128 && s.BatteryFlag != 255
    }
}

#[cfg(not(windows))]
mod imp {
    use super::*;
    pub fn drives() -> Vec<DriveInfo> {
        Vec::new()
    }
    pub fn memory() -> io::Result<MemoryInfo> {
        Err(io::Error::new(io::ErrorKind::Unsupported, "Windows only"))
    }
    pub fn uptime_seconds() -> u64 {
        0
    }
    pub fn running_process_names() -> HashSet<String> {
        HashSet::new()
    }
    pub fn recycle_bin_size() -> io::Result<(u64, u64)> {
        Err(io::Error::new(io::ErrorKind::Unsupported, "Windows only"))
    }
    pub fn empty_recycle_bin() -> io::Result<()> {
        Err(io::Error::new(io::ErrorKind::Unsupported, "Windows only"))
    }
    pub fn has_battery() -> bool {
        false
    }
    pub fn on_battery() -> bool {
        false
    }
}

pub use imp::*;

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn reads_live_system_facts() {
        let drives = drives();
        assert!(
            drives.iter().any(|d| d.is_system && d.total_bytes > 0),
            "system drive missing: {drives:?}"
        );
        let m = memory().unwrap();
        assert!(m.load_percent <= 100 && m.total_bytes > 0);
        assert!(uptime_seconds() > 0);
        let procs = running_process_names();
        assert!(procs.iter().any(|p| p.ends_with(".exe")));
        assert!(recycle_bin_size().is_ok());
    }
}
