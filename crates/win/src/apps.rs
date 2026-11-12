//! Installed apps: list, uninstall with the app's own uninstaller, then a cautious leftovers scan.

use serde::{Deserialize, Serialize};
use smd_core::apps::{Confidence, folder_matches_app, folder_matches_publisher, is_microsoft};
use std::io;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AppKind {
    Desktop,
    Store,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledApp {
    /// `hklm:<key>`, `hklm32:<key>`, `hkcu:<key>` or `appx:<PackageFullName>`.
    pub id: String,
    pub kind: AppKind,
    pub name: String,
    pub publisher: String,
    pub version: String,
    /// `YYYY-MM-DD`
    pub install_date: Option<String>,
    pub size_bytes: Option<u64>,
    pub install_location: Option<String>,
    pub microsoft: bool,
    /// Registered for every user of the PC: the uninstaller will likely ask for admin rights.
    pub machine_wide: bool,
    #[serde(skip)]
    pub uninstall_command: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UninstallResult {
    pub exit_code: Option<u32>,
    /// The app is still listed after the uninstaller closed (cancelled, failed or still finishing).
    pub still_installed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LeftoverKind {
    Folder,
    RegistryKey,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Leftover {
    /// Folder path, or registry key like `HKCU\Software\Vendor\App`.
    pub path: String,
    pub kind: LeftoverKind,
    pub bytes: u64,
    pub confidence: Confidence,
    /// Under Program Files, ProgramData or HKLM: needs the admin helper (Phase 4).
    pub needs_admin: bool,
    /// Ticked when the review opens: only the app's own install folder.
    pub selected: bool,
    /// The folder it was found in, checked again before removal.
    #[serde(skip)]
    pub root: String,
}

fn format_date(s: &str) -> Option<String> {
    smd_core::apps::parse_install_date(s).map(|(y, m, d)| format!("{y:04}-{m:02}-{d:02}"))
}

fn env_path(name: &str) -> Option<PathBuf> {
    std::env::var(name)
        .ok()
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
}

/// Program Files, ProgramData and Windows need admin rights to change.
fn protected_roots() -> Vec<String> {
    [
        "ProgramFiles",
        "ProgramFiles(x86)",
        "ProgramW6432",
        "ProgramData",
        "WINDIR",
    ]
    .iter()
    .filter_map(|v| std::env::var(v).ok())
    .collect()
}

fn needs_admin_path(path: &str) -> bool {
    protected_roots()
        .iter()
        .any(|r| smd_core::safety::same_path(path, r) || smd_core::safety::is_inside(path, r))
}

#[cfg(windows)]
mod imp {
    use super::*;
    use winreg::RegKey;
    use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, KEY_WOW64_64KEY};

    const UNINSTALL: &str = r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall";
    const UNINSTALL32: &str = r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall";

    fn open(prefix: &str) -> Option<(RegKey, &'static str)> {
        match prefix {
            "hklm" => RegKey::predef(HKEY_LOCAL_MACHINE)
                .open_subkey_with_flags(UNINSTALL, KEY_READ | KEY_WOW64_64KEY)
                .ok()
                .map(|k| (k, UNINSTALL)),
            "hklm32" => RegKey::predef(HKEY_LOCAL_MACHINE)
                .open_subkey_with_flags(UNINSTALL32, KEY_READ)
                .ok()
                .map(|k| (k, UNINSTALL32)),
            "hkcu" => RegKey::predef(HKEY_CURRENT_USER)
                .open_subkey_with_flags(UNINSTALL, KEY_READ)
                .ok()
                .map(|k| (k, UNINSTALL)),
            _ => None,
        }
    }

    fn desktop_apps(out: &mut Vec<InstalledApp>) {
        for prefix in ["hklm", "hklm32", "hkcu"] {
            let Some((root, _)) = open(prefix) else {
                continue;
            };
            for key_name in root.enum_keys().flatten() {
                let Ok(k) = root.open_subkey_with_flags(&key_name, KEY_READ) else {
                    continue;
                };
                let s = |n: &str| {
                    k.get_value::<String, _>(n)
                        .ok()
                        .map(|v| v.trim().to_string())
                        .filter(|v| !v.is_empty())
                };
                let d = |n: &str| k.get_value::<u32, _>(n).ok();
                let Some(name) = s("DisplayName") else {
                    continue;
                };
                let uninstall = s("UninstallString");
                if uninstall.is_none()
                    || d("SystemComponent") == Some(1)
                    || s("ParentKeyName").is_some()
                {
                    continue;
                }
                if s("ReleaseType").is_some_and(|r| r.contains("Update") || r.contains("Hotfix")) {
                    continue;
                }
                let publisher = s("Publisher").unwrap_or_default();
                out.push(InstalledApp {
                    id: format!("{prefix}:{key_name}"),
                    kind: AppKind::Desktop,
                    microsoft: is_microsoft(&publisher),
                    publisher,
                    version: s("DisplayVersion").unwrap_or_default(),
                    install_date: s("InstallDate").as_deref().and_then(format_date),
                    size_bytes: d("EstimatedSize")
                        .filter(|kb| *kb > 0)
                        .map(|kb| u64::from(kb) * 1024),
                    install_location: s("InstallLocation")
                        .map(|p| p.trim_matches('"').trim_end_matches('\\').to_string())
                        .filter(|p| p.len() > 3),
                    machine_wide: prefix != "hkcu",
                    uninstall_command: uninstall,
                    name,
                });
            }
        }
    }

    /// True while the app's registry entry still exists.
    pub fn still_registered(id: &str) -> bool {
        let Some((prefix, key)) = id.split_once(':') else {
            return false;
        };
        open(prefix).is_some_and(|(root, _)| root.open_subkey_with_flags(key, KEY_READ).is_ok())
    }

    pub fn list() -> io::Result<Vec<InstalledApp>> {
        let mut out = Vec::new();
        desktop_apps(&mut out);
        match store_apps() {
            Ok(s) => out.extend(s),
            Err(e) => log::warn!("Store apps could not be listed: {e}"),
        }
        // The same app registered twice (32/64-bit views) appears once.
        out.sort_by(|a, b| {
            a.name
                .to_lowercase()
                .cmp(&b.name.to_lowercase())
                .then(a.id.cmp(&b.id))
        });
        out.dedup_by(|a, b| {
            a.kind == b.kind
                && a.name == b.name
                && a.version == b.version
                && a.publisher == b.publisher
        });
        Ok(out)
    }

    /// HKCU registry keys named after the app (`Software\App`, `Software\Publisher\App`).
    pub fn registry_leftovers(app: &InstalledApp) -> Vec<Leftover> {
        let mut out = Vec::new();
        let Ok(sw) = RegKey::predef(HKEY_CURRENT_USER).open_subkey_with_flags("Software", KEY_READ)
        else {
            return out;
        };
        for name in sw.enum_keys().flatten() {
            if folder_matches_app(&name, &app.name) {
                out.push(reg_leftover(format!(r"HKCU\Software\{name}")));
            } else if !app.publisher.is_empty()
                && folder_matches_publisher(&name, &app.publisher)
                && let Ok(pub_key) = sw.open_subkey_with_flags(&name, KEY_READ)
            {
                for sub in pub_key.enum_keys().flatten() {
                    if folder_matches_app(&sub, &app.name) {
                        out.push(reg_leftover(format!(r"HKCU\Software\{name}\{sub}")));
                    }
                }
            }
        }
        out
    }

    fn reg_leftover(path: String) -> Leftover {
        Leftover {
            path,
            kind: LeftoverKind::RegistryKey,
            bytes: 0,
            confidence: Confidence::Medium,
            needs_admin: false,
            selected: false,
            root: r"HKCU\Software".into(),
        }
    }

    /// Exports a key to a .reg file (for undo), then deletes it. Only `HKCU\Software\...` keys.
    pub fn backup_and_delete_key(key: &str, backup: &Path) -> io::Result<()> {
        let sub = key.strip_prefix(r"HKCU\Software\").ok_or_else(|| {
            io::Error::new(io::ErrorKind::PermissionDenied, "only HKCU\\Software keys")
        })?;
        if sub.is_empty()
            || sub.contains(['"', '\0', '\n', '\r'])
            || sub.split('\\').any(|s| s.is_empty() || s == "..")
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "unexpected key name",
            ));
        }
        if let Some(dir) = backup.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let status = std::process::Command::new(reg_exe())
            .arg("export")
            .arg(key)
            .arg(backup)
            .arg("/y")
            .output()?;
        if !status.status.success() || !backup.is_file() {
            return Err(io::Error::other(format!(
                "reg export failed: {}",
                String::from_utf8_lossy(&status.stderr).trim()
            )));
        }
        RegKey::predef(HKEY_CURRENT_USER)
            .open_subkey("Software")?
            .delete_subkey_all(sub)
    }

    /// Puts a key back from its .reg backup.
    pub fn restore_key(backup: &Path) -> io::Result<()> {
        let out = std::process::Command::new(reg_exe())
            .arg("import")
            .arg(backup)
            .output()?;
        if out.status.success() {
            Ok(())
        } else {
            Err(io::Error::other(
                String::from_utf8_lossy(&out.stderr).trim().to_string(),
            ))
        }
    }

    pub fn key_exists(key: &str) -> bool {
        key.strip_prefix(r"HKCU\").is_some_and(|sub| {
            RegKey::predef(HKEY_CURRENT_USER)
                .open_subkey_with_flags(sub, KEY_READ)
                .is_ok()
        })
    }

    fn reg_exe() -> PathBuf {
        env_path("WINDIR")
            .unwrap_or_else(|| PathBuf::from(r"C:\Windows"))
            .join(r"System32\reg.exe")
    }
}

#[cfg(not(windows))]
mod imp {
    use super::*;
    pub fn still_registered(_id: &str) -> bool {
        false
    }
    pub fn list() -> io::Result<Vec<InstalledApp>> {
        Ok(Vec::new())
    }
    pub fn registry_leftovers(_app: &InstalledApp) -> Vec<Leftover> {
        Vec::new()
    }
    pub fn backup_and_delete_key(_key: &str, _backup: &Path) -> io::Result<()> {
        Err(io::Error::new(io::ErrorKind::Unsupported, "Windows only"))
    }
    pub fn restore_key(_backup: &Path) -> io::Result<()> {
        Err(io::Error::new(io::ErrorKind::Unsupported, "Windows only"))
    }
    pub fn key_exists(_key: &str) -> bool {
        false
    }
}

pub use imp::*;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawAppx {
    full: Option<String>,
    name: Option<String>,
    display: Option<String>,
    publisher: Option<String>,
    version: Option<String>,
    location: Option<String>,
}

const APPX: &str = r#"
$ErrorActionPreference = 'Stop'
$p = @(Get-AppxPackage | Where-Object { -not $_.IsFramework -and -not $_.NonRemovable -and $_.SignatureKind -eq 'Store' } | ForEach-Object {
  $d = $null; $pub = $null
  try { $m = Get-AppxPackageManifest -Package $_.PackageFullName; $d = [string]$m.Package.Properties.DisplayName; $pub = [string]$m.Package.Properties.PublisherDisplayName } catch {}
  [pscustomobject]@{ full = [string]$_.PackageFullName; name = [string]$_.Name; display = $d; publisher = $pub; version = [string]$_.Version; location = [string]$_.InstallLocation }
})
ConvertTo-Json -InputObject $p -Depth 4 -Compress
"#;

const APPX_REMOVE: &str = r#"
$ErrorActionPreference = 'Stop'
Remove-AppxPackage -Package $env:SMD_PACKAGE
ConvertTo-Json -InputObject $true -Compress
"#;

/// Store apps for the current user (no admin needed).
pub fn store_apps() -> io::Result<Vec<InstalledApp>> {
    let raw: Vec<RawAppx> =
        crate::powershell::run_json_with(APPX, &[], Some(std::time::Duration::from_secs(90)))?;
    Ok(raw
        .into_iter()
        .filter_map(|r| {
            let full = r.full?;
            let name = r
                .display
                .filter(|d| !d.is_empty() && !d.starts_with("ms-resource:"))
                .or(r.name)
                .unwrap_or_else(|| full.clone());
            let publisher = r
                .publisher
                .filter(|p| !p.starts_with("ms-resource:"))
                .unwrap_or_default();
            Some(InstalledApp {
                id: format!("appx:{full}"),
                kind: AppKind::Store,
                microsoft: is_microsoft(&publisher),
                publisher,
                version: r.version.unwrap_or_default(),
                install_date: None,
                size_bytes: None,
                install_location: r.location.filter(|l| !l.is_empty()),
                machine_wide: false,
                uninstall_command: None,
                name,
            })
        })
        .collect())
}

/// Runs the app's own uninstaller (or removes the Store package) and waits for it.
pub fn uninstall(app: &InstalledApp) -> io::Result<UninstallResult> {
    if let Some(full) = app.id.strip_prefix("appx:") {
        if !smd_core::apps::is_safe_appx_name(full) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "unexpected package name",
            ));
        }
        let _: bool = crate::powershell::run_json_with(
            APPX_REMOVE,
            &[("SMD_PACKAGE", full)],
            Some(std::time::Duration::from_secs(300)),
        )?;
        let still = store_apps()
            .map(|l| l.iter().any(|a| a.id == app.id))
            .unwrap_or(false);
        return Ok(UninstallResult {
            exit_code: Some(0),
            still_installed: still,
        });
    }

    let cmd = app
        .uninstall_command
        .as_deref()
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no uninstaller"))?;
    let (program, args) = if let Some(code) = smd_core::cmdline::msi_product_code(cmd) {
        let msiexec = env_path("WINDIR")
            .unwrap_or_else(|| PathBuf::from(r"C:\Windows"))
            .join(r"System32\msiexec.exe");
        (msiexec.to_string_lossy().into_owned(), format!("/x {code}"))
    } else {
        let expanded = smd_core::cmdline::expand_env(cmd, crate::shell::env_lookup);
        let (p, a) = smd_core::cmdline::split_command(&expanded, |p| Path::new(p).is_file())
            .ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidData, "unreadable uninstall command")
            })?;
        if !Path::new(&p).is_absolute() || !Path::new(&p).is_file() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("uninstaller not found: {p}"),
            ));
        }
        (p, a)
    };
    log::info!("uninstalling {} with {program}", app.name);
    let code = crate::shell::run_and_wait(&program, &args)?;
    // Some uninstallers hand over to a copy of themselves and return at once: give the entry a moment.
    let mut still = still_registered(&app.id);
    for _ in 0..10 {
        if !still {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(500));
        still = still_registered(&app.id);
    }
    Ok(UninstallResult {
        exit_code: Some(code),
        still_installed: still,
    })
}

/// Folders and registry keys that look like they belong to `app`. Read-only.
pub fn find_leftovers(app: &InstalledApp) -> Vec<Leftover> {
    let mut out: Vec<Leftover> = Vec::new();
    let mut push_folder = |path: PathBuf, root: &Path, confidence: Confidence| {
        let p = path.to_string_lossy().into_owned();
        if out.iter().any(|l| smd_core::safety::same_path(&l.path, &p)) || !path.is_dir() {
            return;
        }
        if smd_core::safety::check_removal(&p, &root.to_string_lossy(), false).is_err() {
            return;
        }
        let needs_admin = needs_admin_path(&p);
        out.push(Leftover {
            bytes: crate::walk::measure(&path).bytes,
            kind: LeftoverKind::Folder,
            confidence,
            selected: confidence == Confidence::High && !needs_admin,
            needs_admin,
            root: root.to_string_lossy().into_owned(),
            path: p,
        });
    };

    if app.kind == AppKind::Desktop
        && let Some(loc) = app.install_location.as_deref()
    {
        let loc = PathBuf::from(loc);
        // Some apps list a shared vendor folder as their install location: only a folder named
        // after the app is "high" (ticked); anything else is shown unticked.
        let own = loc
            .file_name()
            .is_some_and(|n| folder_matches_app(&n.to_string_lossy(), &app.name));
        if let Some(parent) = loc.parent() {
            push_folder(
                loc.clone(),
                parent,
                if own {
                    Confidence::High
                } else {
                    Confidence::Medium
                },
            );
        }
    }

    let local = env_path("LOCALAPPDATA");
    let bases: Vec<PathBuf> = [
        env_path("APPDATA"),
        local.clone(),
        local.map(|l| l.join("Programs")),
        env_path("ProgramData"),
        env_path("ProgramFiles"),
        env_path("ProgramFiles(x86)"),
    ]
    .into_iter()
    .flatten()
    .collect();
    for base in &bases {
        let Ok(entries) = std::fs::read_dir(base) else {
            continue;
        };
        for e in entries.flatten() {
            if !e.file_type().is_ok_and(|t| t.is_dir()) {
                continue;
            }
            let name = e.file_name().to_string_lossy().into_owned();
            if folder_matches_app(&name, &app.name) {
                push_folder(e.path(), base, Confidence::Medium);
            } else if !app.publisher.is_empty()
                && folder_matches_publisher(&name, &app.publisher)
                && let Ok(inner) = std::fs::read_dir(e.path())
            {
                for i in inner.flatten() {
                    if i.file_type().is_ok_and(|t| t.is_dir())
                        && folder_matches_app(&i.file_name().to_string_lossy(), &app.name)
                    {
                        push_folder(i.path(), base, Confidence::Medium);
                    }
                }
            }
        }
    }
    // Store apps keep their data under Packages; Windows removes it with the package.
    out.extend(registry_leftovers(app));
    out.sort_by(|a, b| b.confidence.cmp(&a.confidence).then(b.bytes.cmp(&a.bytes)));
    out
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn lists_installed_apps_read_only() {
        let apps = list().unwrap();
        assert!(!apps.is_empty());
        assert!(apps.iter().all(|a| !a.name.is_empty() && !a.id.is_empty()));
        assert!(
            apps.iter()
                .filter(|a| a.kind == AppKind::Desktop)
                .all(|a| a.uninstall_command.is_some())
        );
        assert!(!still_registered(
            "hklm:{00000000-0000-0000-0000-000000000000}-does-not-exist"
        ));
    }

    #[test]
    fn finds_own_test_folders_only() {
        let app = InstalledApp {
            id: "hkcu:smd-test".into(),
            kind: AppKind::Desktop,
            name: "SmdLeftoverTestApp 1.0".into(),
            publisher: "SmdTestVendor".into(),
            version: "1.0".into(),
            install_date: None,
            size_bytes: None,
            install_location: None,
            microsoft: false,
            machine_wide: false,
            uninstall_command: None,
        };
        let local = PathBuf::from(std::env::var("LOCALAPPDATA").unwrap());
        let own = local.join("SmdLeftoverTestApp");
        std::fs::create_dir_all(&own).unwrap();
        std::fs::write(own.join("x.dat"), b"12345").unwrap();
        let found = find_leftovers(&app);
        let _ = std::fs::remove_dir_all(&own);
        let mine: Vec<_> = found
            .iter()
            .filter(|l| l.path.to_lowercase().contains("smdleftovertestapp"))
            .collect();
        assert_eq!(mine.len(), 1, "{found:?}");
        assert_eq!(mine[0].confidence, Confidence::Medium);
        assert!(
            !mine[0].selected,
            "only the install folder is ticked by default"
        );
        assert_eq!(mine[0].bytes, 5);
    }
}
