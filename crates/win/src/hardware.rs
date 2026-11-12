//! Hardware Health: processor, graphics, memory modules, battery report, and the optional
//! sensor helper (`smd-hwmon.exe`, LibreHardwareMonitor) that streams temperatures and fans.

use serde::{Deserialize, Serialize};
use smd_core::hardware::{BatteryInfo, HardwareReading, clean_readings, parse_battery_report};
use std::io::{self, BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CpuInfo {
    pub name: String,
    pub cores: u32,
    pub threads: u32,
    pub max_mhz: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GpuInfo {
    pub name: String,
    pub driver_version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryModule {
    pub slot: String,
    pub bank: String,
    pub capacity_bytes: u64,
    pub speed_mts: u32,
    pub part_number: String,
    pub manufacturer: String,
    /// `DDR4`, `DDR5`... (from SMBIOS), empty if unknown.
    pub kind: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemInfo {
    pub cpu: Option<CpuInfo>,
    pub gpus: Vec<GpuInfo>,
    pub modules: Vec<MemoryModule>,
    /// Memory slots on the board (empty ones included), if Windows knows.
    pub slots: Option<u32>,
}

const INFO: &str = r#"
$ErrorActionPreference = 'Stop'
$c = Get-CimInstance Win32_Processor | Select-Object -First 1
$types = @{ 20 = 'DDR'; 21 = 'DDR2'; 24 = 'DDR3'; 26 = 'DDR4'; 34 = 'DDR5'; 35 = 'LPDDR5' }
$mods = @(Get-CimInstance Win32_PhysicalMemory | ForEach-Object {
  [pscustomobject]@{ slot = [string]$_.DeviceLocator; bank = [string]$_.BankLabel; capacityBytes = [uint64]$_.Capacity; speedMts = [uint32]$_.Speed
    partNumber = ([string]$_.PartNumber).Trim(); manufacturer = ([string]$_.Manufacturer).Trim(); kind = [string]$types[[int]$_.SMBIOSMemoryType] }
})
$arr = Get-CimInstance Win32_PhysicalMemoryArray -ErrorAction SilentlyContinue | Select-Object -First 1
ConvertTo-Json -Depth 4 -Compress -InputObject ([pscustomobject]@{
  cpu = if ($c) { [pscustomobject]@{ name = ([string]$c.Name).Trim(); cores = [uint32]$c.NumberOfCores; threads = [uint32]$c.NumberOfLogicalProcessors; maxMhz = [uint32]$c.MaxClockSpeed } } else { $null }
  gpus = @(Get-CimInstance Win32_VideoController | ForEach-Object { [pscustomobject]@{ name = [string]$_.Name; driverVersion = [string]$_.DriverVersion } })
  modules = $mods
  slots = if ($arr) { [uint32]$arr.MemoryDevices } else { $null }
})
"#;

pub fn system_info() -> io::Result<SystemInfo> {
    crate::powershell::run_json_with(INFO, &[], Some(Duration::from_secs(45)))
}

/// Batteries from `powercfg /batteryreport /xml` (no admin needed). Empty on desktops.
pub fn batteries(temp_dir: &Path) -> io::Result<Vec<BatteryInfo>> {
    if !crate::system::has_battery() {
        return Ok(Vec::new());
    }
    std::fs::create_dir_all(temp_dir)?;
    let out = temp_dir.join("battery-report.xml");
    let windir = std::env::var("WINDIR").unwrap_or_else(|_| r"C:\Windows".into());
    let mut cmd = Command::new(Path::new(&windir).join(r"System32\powercfg.exe"));
    cmd.arg("/batteryreport")
        .arg("/xml")
        .arg("/output")
        .arg(&out)
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    let status = cmd.status()?;
    if !status.success() {
        return Err(io::Error::other(format!("powercfg exited with {status}")));
    }
    let bytes = std::fs::read(&out)?;
    let _ = std::fs::remove_file(&out);
    // The report is UTF-8, sometimes UTF-16 LE with a BOM.
    let text = if bytes.starts_with(&[0xFF, 0xFE]) {
        let (pairs, _) = bytes[2..].as_chunks::<2>();
        let units: Vec<u16> = pairs.iter().map(|c| u16::from_le_bytes(*c)).collect();
        String::from_utf16_lossy(&units)
    } else {
        String::from_utf8_lossy(&bytes).into_owned()
    };
    Ok(parse_battery_report(&text))
}

#[derive(Debug, Clone, PartialEq, Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SensorSnapshot {
    /// The helper runs with admin rights (CPU temperatures and fans available).
    pub elevated: bool,
    pub hardware: Vec<HardwareReading>,
    pub error: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawSnapshot {
    elevated: bool,
    hardware: Vec<HardwareReading>,
    error: Option<String>,
}

/// Parses one line of the helper's output, dropping readings that mean "not available".
pub fn parse_sensor_line(line: &str) -> Option<SensorSnapshot> {
    let raw: RawSnapshot = serde_json::from_str(line.trim()).ok()?;
    Some(SensorSnapshot {
        elevated: raw.elevated,
        hardware: clean_readings(raw.hardware, raw.elevated),
        error: raw.error,
    })
}

/// The running sensor helper. Dropping it closes the helper's stdin, which makes it exit.
pub struct SensorHelper {
    child: Child,
    latest: Arc<Mutex<Option<SensorSnapshot>>>,
}

impl SensorHelper {
    /// Starts `smd-hwmon.exe`. Needs the .NET 8 runtime.
    pub fn start(exe: &Path) -> io::Result<Self> {
        if !exe.is_file() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("sensor helper not found: {}", exe.display()),
            ));
        }
        let mut cmd = Command::new(exe);
        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x0800_0000);
        }
        let mut child = cmd.spawn()?;
        let stdout = child.stdout.take().expect("piped");
        let latest = Arc::new(Mutex::new(None));
        let slot = Arc::clone(&latest);
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let Ok(line) = line else { break };
                if let Some(s) = parse_sensor_line(&line)
                    && let Ok(mut l) = slot.lock()
                {
                    *l = Some(s);
                }
            }
        });
        Ok(SensorHelper { child, latest })
    }

    pub fn latest(&self) -> Option<SensorSnapshot> {
        self.latest.lock().ok().and_then(|l| l.clone())
    }

    /// The helper stopped (crashed or the .NET runtime is missing).
    pub fn exited(&mut self) -> Option<i32> {
        self.child
            .try_wait()
            .ok()
            .flatten()
            .map(|s| s.code().unwrap_or(-1))
    }
}

impl Drop for SensorHelper {
    fn drop(&mut self) {
        drop(self.child.stdin.take());
        // Give it a moment to exit on its own, then make sure.
        for _ in 0..20 {
            if self.child.try_wait().ok().flatten().is_some() {
                return;
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Where the helper is installed: `<resources>\hwmon\smd-hwmon.exe`.
pub fn helper_path(resource_dir: &Path) -> PathBuf {
    resource_dir.join("hwmon").join("smd-hwmon.exe")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_helper_output() {
        let line = r#"{"elevated":false,"hardware":[{"name":"Ryzen","kind":"Cpu","sensors":[{"name":"Tctl","kind":"Temperature","value":0},{"name":"Core #1","kind":"Load","value":9.5}]}]}"#;
        let s = parse_sensor_line(line).unwrap();
        assert!(!s.elevated);
        assert_eq!(s.hardware[0].sensors.len(), 1, "0 °C means not available");
        assert!(parse_sensor_line("not json").is_none());
    }

    #[cfg(windows)]
    #[test]
    fn reads_system_info() {
        let info = system_info().unwrap();
        assert!(info.cpu.as_ref().is_some_and(|c| c.threads > 0));
        assert!(batteries(&std::env::temp_dir().join("smd-test")).is_ok());
    }

    /// Runs the real helper when it has been built (`npm run hwmon`).
    #[cfg(windows)]
    #[test]
    fn helper_streams_and_stops() {
        let exe = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join(r"..\..\src-tauri\resources\hwmon\smd-hwmon.exe");
        if !exe.is_file() || std::env::var_os("CI").is_some() {
            return;
        }
        let mut h = SensorHelper::start(&exe).unwrap();
        for _ in 0..100 {
            if h.latest().is_some() {
                break;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        assert!(h.latest().is_some(), "no reading within 10 s");
        assert!(h.exited().is_none());
        drop(h);
    }
}
