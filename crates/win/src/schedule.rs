//! Scheduled Smart Care: one Task Scheduler task for your account that runs
//! `smd.exe --smart-care --silent` (scan only, never deletes). Removed by the uninstallers.

use std::io;
use std::path::Path;
use std::time::Duration;

pub const TASK_NAME: &str = "Scrub My Disks Smart Care";

const REGISTER: &str = r#"
$ErrorActionPreference = 'Stop'
$action = New-ScheduledTaskAction -Execute $env:SMD_EXE -Argument '--smart-care --silent'
$at = [datetime]::Today.AddHours([int]$env:SMD_HOUR)
if ($env:SMD_FREQ -eq 'daily') { $trigger = New-ScheduledTaskTrigger -Daily -At $at }
else { $trigger = New-ScheduledTaskTrigger -Weekly -DaysOfWeek $env:SMD_DAY -At $at }
$settings = New-ScheduledTaskSettingsSet -StartWhenAvailable -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries -ExecutionTimeLimit (New-TimeSpan -Minutes 30)
$principal = New-ScheduledTaskPrincipal -UserId ([Security.Principal.WindowsIdentity]::GetCurrent().Name) -LogonType Interactive -RunLevel Limited
Register-ScheduledTask -TaskName $env:SMD_TASK -Action $action -Trigger $trigger -Settings $settings -Principal $principal -Description 'Scrub My Disks: Smart Care scan (read-only, shows a notification).' -Force | Out-Null
'true'
"#;

const UNREGISTER: &str = r#"
$t = Get-ScheduledTask -TaskName $env:SMD_TASK -ErrorAction SilentlyContinue
if ($t) { Unregister-ScheduledTask -TaskName $env:SMD_TASK -Confirm:$false }
'true'
"#;

const EXISTS: &str = r#"
if (Get-ScheduledTask -TaskName $env:SMD_TASK -ErrorAction SilentlyContinue) { 'true' } else { 'false' }
"#;

const DAYS: [&str; 7] = [
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
    "Sunday",
];

/// `day` is 0 (Monday) to 6 (Sunday); `hour` 0 to 23. `daily` ignores `day`.
pub fn register(exe: &Path, daily: bool, day: u8, hour: u8) -> io::Result<()> {
    if day > 6 || hour > 23 {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "bad schedule"));
    }
    let exe = exe.to_string_lossy();
    let hour = hour.to_string();
    crate::powershell::run_text(
        REGISTER,
        &[
            ("SMD_EXE", &exe),
            ("SMD_FREQ", if daily { "daily" } else { "weekly" }),
            ("SMD_DAY", DAYS[day as usize]),
            ("SMD_HOUR", &hour),
            ("SMD_TASK", TASK_NAME),
        ],
        Some(Duration::from_secs(60)),
    )
    .map(|_| ())
}

pub fn unregister() -> io::Result<()> {
    crate::powershell::run_text(
        UNREGISTER,
        &[("SMD_TASK", TASK_NAME)],
        Some(Duration::from_secs(60)),
    )
    .map(|_| ())
}

pub fn exists() -> io::Result<bool> {
    let out = crate::powershell::run_text(
        EXISTS,
        &[("SMD_TASK", TASK_NAME)],
        Some(Duration::from_secs(30)),
    )?;
    Ok(out.trim() == "true")
}
