//! Drive Organizer audit (read-only): which drive is which, where the games are, and what
//! doesn't fit the drive-layout policy.

use serde::Serialize;
use smd_core::organizer::{
    CACHE_MIN_BYTES, CACHES, Finding, Launcher, Method, PERSONAL_MIN_BYTES, Policy, Reason, Role,
    SeenVolume, Severity, gaming_root_paths, is_personal_file, rate_game, steam_app,
    steam_libraries,
};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use crate::system::{DriveInfo, DriveKind};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Game {
    pub launcher: Launcher,
    pub name: String,
    pub path: String,
    pub drive: String,
    pub bytes: u64,
}

/// `\\?\Volume{...}\` of a drive root like `C:\`.
#[cfg(windows)]
pub fn volume_guid(root: &str) -> Option<String> {
    use windows::Win32::Storage::FileSystem::GetVolumeNameForVolumeMountPointW;
    use windows::core::HSTRING;
    let mut buf = [0u16; 64];
    unsafe { GetVolumeNameForVolumeMountPointW(&HSTRING::from(root), &mut buf) }.ok()?;
    let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    Some(String::from_utf16_lossy(&buf[..end]))
}

#[cfg(not(windows))]
pub fn volume_guid(_root: &str) -> Option<String> {
    None
}

/// Fixed drives with their volume GUID and the serial of the disk they are on.
pub fn seen_volumes(drives: &[DriveInfo], disks: &[crate::disks::PhysicalDisk]) -> Vec<SeenVolume> {
    drives
        .iter()
        .filter(|d| d.kind == DriveKind::Fixed)
        .map(|d| SeenVolume {
            letter: d.letter.clone(),
            volume_guid: volume_guid(&d.root),
            disk_serial: disks
                .iter()
                .find(|k| k.letters.iter().any(|l| l.eq_ignore_ascii_case(&d.letter)))
                .map(|k| k.serial.clone())
                .filter(|s| !s.is_empty()),
        })
        .collect()
}

fn letter_of(path: &str) -> String {
    path.get(0..2)
        .map(|s| s.to_ascii_uppercase())
        .unwrap_or_default()
}

fn norm(p: &str) -> String {
    smd_core::safety::normalize(p)
}

/// Steam games from every library in `libraryfolders.vdf`, sizes from the app manifests.
pub fn steam_games() -> Vec<Game> {
    let Some(steam) = steam_path() else {
        return Vec::new();
    };
    let Ok(vdf) = fs::read_to_string(steam.join(r"steamapps\libraryfolders.vdf")) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for lib in steam_libraries(&vdf) {
        let apps = Path::new(&lib).join("steamapps");
        let Ok(entries) = fs::read_dir(&apps) else {
            continue;
        };
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().to_lowercase();
            if !(name.starts_with("appmanifest_") && name.ends_with(".acf")) {
                continue;
            }
            let Some(app) = fs::read_to_string(e.path())
                .ok()
                .as_deref()
                .and_then(steam_app)
            else {
                continue;
            };
            // Steam's own shared runtimes (Steamworks Common Redistributables) are not games.
            if app.app_id == "228980" || app.name.to_lowercase().contains("redistributable") {
                continue;
            }
            let path = apps.join("common").join(&app.install_dir);
            if !path.is_dir() {
                continue;
            }
            let p = path.to_string_lossy().into_owned();
            out.push(Game {
                launcher: Launcher::Steam,
                name: app.name,
                drive: letter_of(&p),
                path: p,
                bytes: app.size_on_disk,
            });
        }
    }
    out
}

#[cfg(windows)]
fn steam_path() -> Option<PathBuf> {
    let k = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER)
        .open_subkey(r"Software\Valve\Steam")
        .ok()?;
    let p: String = k.get_value("SteamPath").ok()?;
    Some(PathBuf::from(p.replace('/', "\\")))
}

#[cfg(not(windows))]
fn steam_path() -> Option<PathBuf> {
    None
}

/// Epic Games from the launcher's `.item` manifests.
pub fn epic_games() -> Vec<Game> {
    let dir = std::env::var("ProgramData")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(r"C:\ProgramData"));
    let Ok(entries) = fs::read_dir(dir.join(r"Epic\EpicGamesLauncher\Data\Manifests")) else {
        return Vec::new();
    };
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for e in entries.flatten() {
        if !e
            .file_name()
            .to_string_lossy()
            .to_lowercase()
            .ends_with(".item")
        {
            continue;
        }
        let Some(v) = fs::read_to_string(e.path())
            .ok()
            .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
        else {
            continue;
        };
        if v["bIsIncompleteInstall"].as_bool() == Some(true) {
            continue;
        }
        let Some(loc) = v["InstallLocation"].as_str().map(|s| s.replace('/', "\\")) else {
            continue;
        };
        if !Path::new(&loc).is_dir() || !seen.insert(norm(&loc)) {
            continue;
        }
        let bytes = v["InstallSize"].as_u64().unwrap_or(0);
        out.push(Game {
            launcher: Launcher::Epic,
            name: v["DisplayName"].as_str().unwrap_or_default().to_string(),
            drive: letter_of(&loc),
            path: loc,
            bytes,
        });
    }
    out
}

/// GOG Galaxy games from the registry (DLCs share the game's folder and are counted once).
#[cfg(windows)]
pub fn gog_games() -> Vec<Game> {
    use winreg::RegKey;
    use winreg::enums::{HKEY_LOCAL_MACHINE, KEY_READ};
    let Ok(root) = RegKey::predef(HKEY_LOCAL_MACHINE)
        .open_subkey_with_flags(r"SOFTWARE\WOW6432Node\GOG.com\Games", KEY_READ)
    else {
        return Vec::new();
    };
    let mut seen = HashSet::new();
    let mut found: Vec<(String, String)> = Vec::new();
    for k in root.enum_keys().flatten() {
        let Ok(g) = root.open_subkey_with_flags(&k, KEY_READ) else {
            continue;
        };
        let (Ok(name), Ok(path)) = (
            g.get_value::<String, _>("gameName"),
            g.get_value::<String, _>("path"),
        ) else {
            continue;
        };
        if Path::new(&path).is_dir() && seen.insert(norm(&path)) {
            found.push((name, path));
        }
    }
    // Keep the shortest name for a folder (the base game, not "...: Phantom Liberty").
    found.sort_by_key(|(n, _)| n.len());
    found
        .into_iter()
        .map(|(name, path)| Game {
            launcher: Launcher::Gog,
            name,
            drive: letter_of(&path),
            bytes: crate::walk::measure(Path::new(&path)).bytes,
            path,
        })
        .collect()
}

#[cfg(not(windows))]
pub fn gog_games() -> Vec<Game> {
    Vec::new()
}

/// Xbox app games: each drive's `.GamingRoot` names the library folders.
pub fn xbox_games(drives: &[DriveInfo]) -> Vec<Game> {
    let mut out = Vec::new();
    for d in drives.iter().filter(|d| d.kind == DriveKind::Fixed) {
        let Ok(bytes) = fs::read(Path::new(&d.root).join(".GamingRoot")) else {
            continue;
        };
        for rel in gaming_root_paths(&bytes) {
            let lib = Path::new(&d.root).join(rel.trim_start_matches('\\'));
            let Ok(entries) = fs::read_dir(&lib) else {
                continue;
            };
            for e in entries.flatten() {
                if e.file_type().is_ok_and(|t| t.is_dir() && !t.is_symlink()) {
                    let m = crate::walk::measure(&e.path());
                    let p = e.path().to_string_lossy().into_owned();
                    out.push(Game {
                        launcher: Launcher::Xbox,
                        name: e.file_name().to_string_lossy().into_owned(),
                        drive: letter_of(&p),
                        path: p,
                        bytes: m.bytes,
                    });
                }
            }
        }
    }
    out
}

pub fn all_games(drives: &[DriveInfo]) -> Vec<Game> {
    let mut v = steam_games();
    v.extend(epic_games());
    v.extend(gog_games());
    v.extend(xbox_games(drives));
    v.sort_by_key(|g| std::cmp::Reverse(g.bytes));
    v
}

fn id_for(path: &str) -> String {
    let h = blake3::hash(norm(path).as_bytes());
    h.to_hex()[..16].to_string()
}

fn target_for(policy: &Policy, role: Role) -> Option<String> {
    policy.drive_for(role).map(str::to_string)
}

fn game_method(l: Launcher) -> Method {
    match l {
        Launcher::Steam | Launcher::Xbox => Method::LauncherMove { launcher: l },
        Launcher::Epic | Launcher::Gog => Method::Reinstall { launcher: l },
    }
}

/// Folders on the system drive that are Windows' or programs': never searched for personal files.
const SYSTEM_TOP: &[&str] = &[
    "windows",
    "program files",
    "program files (x86)",
    "programdata",
    "users",
    "$recycle.bin",
    "system volume information",
    "recovery",
    "$winreagent",
    "$windows.~bt",
    "$windows.~ws",
    "boot",
    "efi",
    "perflogs",
    "msocache",
    "intel",
    "amd",
    "nvidia",
    "drivers",
    "onedrivetemp",
    "xboxgames",
    "inetpub",
];

fn walk_personal(dir: &Path, out: &mut Vec<(PathBuf, u64)>, skip: &HashSet<String>, depth: u32) {
    if depth > 40 {
        return;
    }
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let Ok(ft) = e.file_type() else { continue };
        if ft.is_symlink() {
            continue;
        }
        let p = e.path();
        if ft.is_dir() {
            if !skip.contains(&norm(&p.to_string_lossy())) {
                walk_personal(&p, out, skip, depth + 1);
            }
        } else if let Ok(m) = e.metadata()
            && m.len() >= PERSONAL_MIN_BYTES
            && is_personal_file(&e.file_name().to_string_lossy())
        {
            out.push((p, m.len()));
        }
    }
}

/// Documents, media and archives of 100 MB or more on the system drive, outside Windows,
/// programs and AppData. Game folders are skipped (they are reported as games).
pub fn personal_files_on(
    system_root: &Path,
    profile: &Path,
    game_dirs: &[String],
) -> Vec<(PathBuf, u64)> {
    let mut skip: HashSet<String> = game_dirs.iter().map(|g| norm(g)).collect();
    skip.insert(norm(&profile.join("AppData").to_string_lossy()));
    let mut out = Vec::new();
    if norm(&profile.to_string_lossy()).starts_with(&norm(&system_root.to_string_lossy())) {
        walk_personal(profile, &mut out, &skip, 0);
    }
    if let Ok(entries) = fs::read_dir(system_root) {
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().to_lowercase();
            if e.file_type().is_ok_and(|t| t.is_dir() && !t.is_symlink())
                && !SYSTEM_TOP.contains(&name.as_str())
                && !name.starts_with('$')
            {
                walk_personal(&e.path(), &mut out, &skip, 0);
            }
        }
    }
    out.sort_by_key(|(_, b)| std::cmp::Reverse(*b));
    out
}

/// Everything on the disposable drive outside Scratch and Quarantine.
pub fn data_on_disposable(root: &Path) -> Vec<(PathBuf, u64)> {
    let Ok(entries) = fs::read_dir(root) else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter(|e| {
            let n = e.file_name().to_string_lossy().to_lowercase();
            !matches!(
                n.as_str(),
                "scratch" | "quarantine" | "$recycle.bin" | "system volume information"
            ) && !n.starts_with('$')
                && !e.file_type().is_ok_and(|t| t.is_symlink())
        })
        .map(|e| {
            let m = crate::walk::measure(&e.path());
            (e.path(), m.bytes)
        })
        .collect()
}

/// Runs the whole audit. Read-only.
pub fn audit(policy: &Policy, drives: &[DriveInfo], games: &[Game]) -> Vec<Finding> {
    let mut out = Vec::new();
    for g in games {
        if let Some((reason, severity, wanted)) =
            rate_game(policy, policy.role_of(&g.drive), g.bytes)
        {
            out.push(Finding {
                id: id_for(&g.path),
                name: g.name.clone(),
                path: g.path.clone(),
                drive: g.drive.clone(),
                bytes: g.bytes,
                severity,
                reason,
                target_drive: target_for(policy, wanted),
                method: game_method(g.launcher),
            });
        }
    }

    let profile = std::env::var("USERPROFILE").map(PathBuf::from).ok();
    if let (Some(system), Some(profile)) = (policy.drive_for(Role::System), profile.as_ref()) {
        let root = PathBuf::from(format!("{system}\\"));
        let game_dirs: Vec<String> = games.iter().map(|g| g.path.clone()).collect();
        for (p, bytes) in personal_files_on(&root, profile, &game_dirs) {
            let s = p.to_string_lossy().into_owned();
            out.push(Finding {
                id: id_for(&s),
                name: p
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                drive: letter_of(&s),
                path: s,
                bytes,
                severity: Severity::Warning,
                reason: Reason::PersonalFilesOnSystemDrive,
                target_drive: target_for(policy, Role::LightGames),
                method: Method::CopyVerify,
            });
        }
        if policy.role_of(&letter_of(&profile.to_string_lossy())) == Role::System {
            for c in CACHES {
                let p = profile.join(c.path);
                if !p.is_dir() {
                    continue;
                }
                let bytes = crate::walk::measure(&p).bytes;
                if bytes < CACHE_MIN_BYTES {
                    continue;
                }
                let s = p.to_string_lossy().into_owned();
                out.push(Finding {
                    id: id_for(&s),
                    name: c.name.to_string(),
                    drive: letter_of(&s),
                    path: s,
                    bytes,
                    severity: Severity::Warning,
                    reason: Reason::CacheOnSystemDrive,
                    target_drive: target_for(policy, Role::HeavyGames),
                    method: Method::RelocateCache {
                        env_var: c.env_var.to_string(),
                    },
                });
            }
        }
    }

    if let Some(s) = policy.drive_for(Role::Disposable)
        && drives.iter().any(|d| d.letter.eq_ignore_ascii_case(s))
    {
        for (p, bytes) in data_on_disposable(Path::new(&format!("{s}\\"))) {
            let st = p.to_string_lossy().into_owned();
            out.push(Finding {
                id: id_for(&st),
                name: p
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                drive: letter_of(&st),
                path: st,
                bytes,
                severity: Severity::Problem,
                reason: Reason::MaybeOnlyCopyOnDisposable,
                target_drive: target_for(policy, Role::LightGames),
                method: Method::CopyVerify,
            });
        }
    }
    out.sort_by(|a, b| {
        b.severity_rank()
            .cmp(&a.severity_rank())
            .then(b.bytes.cmp(&a.bytes))
    });
    out
}

trait Rank {
    fn severity_rank(&self) -> u8;
}
impl Rank for Finding {
    fn severity_rank(&self) -> u8 {
        match self.severity {
            Severity::Problem => 2,
            Severity::Warning => 1,
            Severity::Info => 0,
        }
    }
}

/// Where a finding goes: `<target>\From C\<rest of the path>` for files, `<target>\Caches\<name>` for caches.
pub fn target_path(f: &Finding, target_drive: &str) -> String {
    match &f.method {
        Method::RelocateCache { .. } => {
            let leaf = Path::new(&f.path)
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            let tool = CACHES
                .iter()
                .find(|c| f.path.to_lowercase().ends_with(&c.path.to_lowercase()))
                .map_or(leaf.as_str(), |c| c.id);
            format!(r"{target_drive}\Caches\{tool}")
        }
        _ => {
            let from = f.drive.trim_end_matches(':');
            let rest = f.path.get(3..).unwrap_or_default();
            format!(r"{target_drive}\From {from}\{rest}")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn finds_personal_files_but_not_appdata_or_games() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("C");
        let profile = root.join(r"Users\me");
        let big = |p: &Path| {
            fs::create_dir_all(p.parent().unwrap()).unwrap();
            let f = fs::File::create(p).unwrap();
            f.set_len(PERSONAL_MIN_BYTES).unwrap(); // sparse: no real disk use
        };
        big(&profile.join(r"Stuff\film.mkv"));
        big(&profile.join(r"AppData\Local\cache.mp4"));
        big(&root.join(r"Games\MyGame\intro.mp4"));
        big(&root.join(r"Archive\old.zip"));
        big(&root.join(r"Windows\x.mp4"));
        fs::File::create(profile.join(r"Stuff\small.mp4"))
            .unwrap()
            .write_all(b"x")
            .unwrap();
        let games = vec![root.join(r"Games\MyGame").to_string_lossy().into_owned()];
        let found = personal_files_on(&root, &profile, &games);
        let names: Vec<String> = found
            .iter()
            .map(|(p, _)| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(found.len(), 2, "{names:?}");
        assert!(names.contains(&"film.mkv".into()) && names.contains(&"old.zip".into()));
    }

    #[test]
    fn disposable_drive_data_skips_scratch_and_quarantine() {
        let dir = tempfile::tempdir().unwrap();
        for d in ["Scratch", "Quarantine", "Photos backup"] {
            fs::create_dir_all(dir.path().join(d)).unwrap();
        }
        fs::write(dir.path().join(r"Photos backup\a.jpg"), b"12345").unwrap();
        let found = data_on_disposable(dir.path());
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].1, 5);
    }

    #[test]
    fn target_paths() {
        let f = Finding {
            id: "1".into(),
            name: "film.mkv".into(),
            path: r"C:\Users\me\Stuff\film.mkv".into(),
            drive: "C:".into(),
            bytes: 1,
            severity: Severity::Warning,
            reason: Reason::PersonalFilesOnSystemDrive,
            target_drive: Some("G:".into()),
            method: Method::CopyVerify,
        };
        assert_eq!(target_path(&f, "G:"), r"G:\From C\Users\me\Stuff\film.mkv");
        let c = Finding {
            path: r"C:\Users\me\.cache\huggingface".into(),
            method: Method::RelocateCache {
                env_var: "HF_HOME".into(),
            },
            ..f
        };
        assert_eq!(target_path(&c, "X:"), r"X:\Caches\huggingface");
    }

    #[cfg(windows)]
    #[test]
    fn reads_live_layout() {
        let drives = crate::system::drives();
        let sys = drives.iter().find(|d| d.is_system).unwrap();
        assert!(volume_guid(&sys.root).unwrap().starts_with(r"\\?\Volume{"));
        let games = all_games(&drives);
        assert!(
            games
                .iter()
                .all(|g| !g.path.is_empty() && g.drive.len() == 2)
        );
    }
}
