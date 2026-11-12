//! Toolbox: which catalog tools are installed (`winget export`), install or update them
//! (`winget install/upgrade`), and open them (Start menu entry or the program winget unpacked).
//! Uses the internet only for install and update.

use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use smd_core::cleanup::glob_match;
use smd_core::toolbox::Tool;

fn winget() -> Command {
    let mut cmd = Command::new("winget.exe");
    cmd.stdin(Stdio::null());
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    cmd
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct ExportPackage {
    package_identifier: String,
    #[serde(default)]
    version: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct ExportSource {
    #[serde(default)]
    packages: Vec<ExportPackage>,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Export {
    #[serde(default)]
    sources: Vec<ExportSource>,
}

/// Parses `winget export` JSON into lower-case id -> version.
pub fn parse_export(json: &str) -> io::Result<HashMap<String, Option<String>>> {
    let e: Export = serde_json::from_str(json.trim_start_matches('\u{feff}'))
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    Ok(e.sources
        .into_iter()
        .flat_map(|s| s.packages)
        .map(|p| (p.package_identifier.to_lowercase(), p.version))
        .collect())
}

/// Installed winget packages (about 20 s: winget checks every program).
pub fn installed(temp_dir: &Path) -> io::Result<HashMap<String, Option<String>>> {
    let file = temp_dir.join(format!("winget-export-{}.json", std::process::id()));
    let status = winget()
        .args(["export", "-o"])
        .arg(&file)
        .args([
            "--include-versions",
            "--accept-source-agreements",
            "--disable-interactivity",
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?;
    let text = std::fs::read_to_string(&file);
    let _ = std::fs::remove_file(&file);
    match text {
        Ok(t) => parse_export(&t),
        Err(e) if !status.success() => Err(io::Error::other(format!(
            "winget export failed ({}): {e}",
            status.code().unwrap_or(-1)
        ))),
        Err(e) => Err(e),
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WingetResult {
    pub ok: bool,
    pub code: i32,
    /// Last lines of winget's output, for the error details.
    pub output: String,
}

/// Exit codes winget uses for "nothing to do".
const NO_UPGRADE: i32 = -1978335189; // APPINSTALLER_CLI_ERROR_UPDATE_NOT_APPLICABLE
const ALREADY_INSTALLED: i32 = -1978335135; // APPINSTALLER_CLI_ERROR_PACKAGE_ALREADY_INSTALLED

fn run(verb: &str, id: &str) -> io::Result<WingetResult> {
    if !smd_core::apps::is_safe_package_id(id) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "bad package id",
        ));
    }
    let out = winget()
        .args([
            verb,
            "--id",
            id,
            "--exact",
            "--source",
            "winget",
            "--silent",
            "--accept-package-agreements",
            "--accept-source-agreements",
            "--disable-interactivity",
        ])
        .output()?;
    let code = out.status.code().unwrap_or(-1);
    let text = String::from_utf8_lossy(&out.stdout);
    let tail: Vec<&str> = text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && l.chars().any(char::is_alphanumeric))
        .collect();
    let output = tail[tail.len().saturating_sub(6)..].join("\n");
    Ok(WingetResult {
        ok: code == 0 || code == NO_UPGRADE || code == ALREADY_INSTALLED,
        code,
        output,
    })
}

pub fn install(id: &str) -> io::Result<WingetResult> {
    run("install", id)
}

pub fn upgrade(id: &str) -> io::Result<WingetResult> {
    run("upgrade", id)
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct StartApp {
    pub name: String,
    #[serde(rename = "AppID")]
    pub app_id: String,
}

const START_APPS: &str = r#"
$ErrorActionPreference = 'Stop'
ConvertTo-Json -InputObject @(Get-StartApps | Select-Object Name, AppID) -Compress
"#;

/// The Start menu's app list.
pub fn start_apps() -> io::Result<Vec<StartApp>> {
    crate::powershell::run_json_with(START_APPS, &[], Some(Duration::from_secs(30)))
}

fn find_exe_in(dir: &Path, globs: &[String], depth: u32) -> Option<PathBuf> {
    let entries: Vec<_> = std::fs::read_dir(dir).ok()?.flatten().collect();
    for g in globs {
        for e in &entries {
            let name = e.file_name().to_string_lossy().into_owned();
            if e.file_type().is_ok_and(|t| t.is_file() || t.is_symlink()) && glob_match(g, &name) {
                return Some(e.path());
            }
        }
    }
    if depth == 0 {
        return None;
    }
    entries
        .iter()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .find_map(|e| find_exe_in(&e.path(), globs, depth - 1))
}

/// The program winget unpacked for a portable tool: its command alias, or its package folder.
pub fn portable_exe(tool: &Tool) -> Option<PathBuf> {
    if tool.open.exe.is_empty() {
        return None;
    }
    let base = PathBuf::from(std::env::var("LOCALAPPDATA").ok()?).join(r"Microsoft\WinGet");
    if let Some(p) = find_exe_in(&base.join("Links"), &tool.open.exe, 0) {
        return Some(p);
    }
    let prefix = format!("{}_", tool.winget_id.to_lowercase());
    let packages = std::fs::read_dir(base.join("Packages")).ok()?;
    packages
        .flatten()
        .filter(|e| {
            e.file_name()
                .to_string_lossy()
                .to_lowercase()
                .starts_with(&prefix)
        })
        .find_map(|e| find_exe_in(&e.path(), &tool.open.exe, 2))
}

/// A Start menu name matches the catalog name exactly, or with a suffix like "PowerToys (Preview)".
pub fn start_app_matches(have: &str, wanted: &str) -> bool {
    let (h, w) = (have.to_lowercase(), wanted.to_lowercase());
    h == w || h.starts_with(&format!("{w} ("))
}

/// Opens a tool: its Start menu entry if there is one, else the program winget unpacked.
pub fn open(tool: &Tool, apps: &[StartApp]) -> io::Result<()> {
    if let Some(name) = &tool.open.start_app
        && let Some(app) = apps.iter().find(|a| start_app_matches(&a.name, name))
    {
        Command::new("explorer.exe")
            .arg(format!(r"shell:AppsFolder\{}", app.app_id))
            .spawn()?;
        return Ok(());
    }
    let exe = portable_exe(tool).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            format!("{} has no program to open", tool.name),
        )
    })?;
    Command::new(exe).spawn().map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_winget_export() {
        let json = "\u{feff}{\"Sources\":[{\"Packages\":[{\"PackageIdentifier\":\"7zip.7zip\",\"Version\":\"23.01\"},{\"PackageIdentifier\":\"Microsoft.PowerToys\"}],\"SourceDetails\":{\"Name\":\"winget\"}}]}";
        let m = parse_export(json).unwrap();
        assert_eq!(m.get("7zip.7zip"), Some(&Some("23.01".to_string())));
        assert_eq!(m.get("microsoft.powertoys"), Some(&None));
    }

    #[test]
    fn finds_programs_by_glob() {
        let dir = tempfile::tempdir().unwrap();
        let nested = dir.path().join("pkg").join("bin");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::write(nested.join("krokiet_12.exe"), b"").unwrap();
        let found = find_exe_in(dir.path(), &["krokiet_*.exe".to_string()], 2).unwrap();
        assert!(found.ends_with("krokiet_12.exe"));
        assert!(find_exe_in(dir.path(), &["other.exe".to_string()], 2).is_none());
    }

    #[test]
    fn start_menu_names() {
        assert!(start_app_matches("PowerToys (Preview)", "PowerToys"));
        assert!(start_app_matches("crystaldiskinfo", "CrystalDiskInfo"));
        assert!(!start_app_matches("PowerToys Run", "PowerToys"));
    }
}
