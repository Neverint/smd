//! Drive Organizer: the drive-layout policy, the audit rules, launcher manifest parsing and
//! the migration planner. Pure logic: the Windows side measures, this side decides.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::health::Status;

pub const DEFAULT_POLICY_JSON: &str = include_str!("../../../rules/drive-policy.json");
pub const GB: u64 = 1024 * 1024 * 1024;

/// What a drive is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Role {
    /// Windows, programs, the user profile, pagefile, TEMP. No games, documents or media.
    System,
    /// Games over the threshold, Downloads.
    HeavyGames,
    /// Games under the threshold, documents and media.
    LightGames,
    /// Scratch and quarantine only: nothing unique or important.
    Disposable,
    /// No rule.
    Unassigned,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DriveRule {
    pub letter: String,
    pub role: Role,
    /// `\\?\Volume{...}\`, filled in on first run so a changed letter is noticed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub volume_guid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disk_serial: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Policy {
    pub version: u32,
    pub game_threshold_gb: u32,
    pub drives: Vec<DriveRule>,
}

impl Policy {
    pub fn default_policy() -> Policy {
        serde_json::from_str(DEFAULT_POLICY_JSON)
            .expect("built-in drive policy is validated by tests")
    }

    pub fn threshold_bytes(&self) -> u64 {
        u64::from(self.game_threshold_gb) * GB
    }

    pub fn role_of(&self, letter: &str) -> Role {
        self.drives
            .iter()
            .find(|d| d.letter.eq_ignore_ascii_case(letter))
            .map_or(Role::Unassigned, |d| d.role)
    }

    /// First drive with this role.
    pub fn drive_for(&self, role: Role) -> Option<&str> {
        self.drives
            .iter()
            .find(|d| d.role == role)
            .map(|d| d.letter.as_str())
    }

    /// Checks a policy the user edited.
    pub fn validate(&self) -> Result<(), String> {
        if self.version != 1 {
            return Err(format!("unsupported policy version {}", self.version));
        }
        if !(1..=2000).contains(&self.game_threshold_gb) {
            return Err("game threshold must be between 1 and 2000 GB".into());
        }
        let mut seen = std::collections::HashSet::new();
        for d in &self.drives {
            let l = d.letter.to_ascii_uppercase();
            if l.len() != 2 || !l.as_bytes()[0].is_ascii_uppercase() || l.as_bytes()[1] != b':' {
                return Err(format!("not a drive letter: {}", d.letter));
            }
            if !seen.insert(l) {
                return Err(format!("{} is listed twice", d.letter));
            }
        }
        Ok(())
    }
}

/// A drive as Windows sees it now, for matching against the policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeenVolume {
    pub letter: String,
    pub volume_guid: Option<String>,
    pub disk_serial: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum PolicyNotice {
    /// The drive this rule was made for now has another letter: the rule follows it.
    LetterChanged { from: String, to: String },
    /// No drive matches this rule right now (unplugged or removed).
    Missing { letter: String },
}

/// Matches the policy to the drives present now: by volume GUID first (so a changed letter
/// is followed), then by letter for rules that have no GUID yet (first run). Fills in GUIDs and
/// serials. Returns the updated policy and what changed.
pub fn bind_policy(policy: &Policy, seen: &[SeenVolume]) -> (Policy, Vec<PolicyNotice>) {
    let mut out = policy.clone();
    let mut notices = Vec::new();
    for rule in &mut out.drives {
        let by_guid = rule.volume_guid.as_deref().and_then(|g| {
            seen.iter().find(|v| {
                v.volume_guid
                    .as_deref()
                    .is_some_and(|vg| vg.eq_ignore_ascii_case(g))
            })
        });
        let found = by_guid.or_else(|| {
            rule.volume_guid
                .is_none()
                .then(|| {
                    seen.iter()
                        .find(|v| v.letter.eq_ignore_ascii_case(&rule.letter))
                })
                .flatten()
        });
        match found {
            Some(v) => {
                if !v.letter.eq_ignore_ascii_case(&rule.letter) {
                    notices.push(PolicyNotice::LetterChanged {
                        from: rule.letter.clone(),
                        to: v.letter.clone(),
                    });
                    rule.letter = v.letter.to_ascii_uppercase();
                }
                if rule.volume_guid.is_none() {
                    rule.volume_guid = v.volume_guid.clone();
                }
                if v.disk_serial.is_some() {
                    rule.disk_serial = v.disk_serial.clone();
                }
            }
            None => notices.push(PolicyNotice::Missing {
                letter: rule.letter.clone(),
            }),
        }
    }
    (out, notices)
}

/* ---------- Valve KeyValues (libraryfolders.vdf, appmanifest_*.acf) ---------- */

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Kv {
    Str(String),
    Map(BTreeMap<String, Kv>),
}

impl Kv {
    pub fn get(&self, key: &str) -> Option<&Kv> {
        match self {
            Kv::Map(m) => m
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case(key))
                .map(|(_, v)| v),
            Kv::Str(_) => None,
        }
    }
    pub fn str(&self, key: &str) -> Option<&str> {
        match self.get(key)? {
            Kv::Str(s) => Some(s),
            Kv::Map(_) => None,
        }
    }
    pub fn map(&self) -> Option<&BTreeMap<String, Kv>> {
        match self {
            Kv::Map(m) => Some(m),
            Kv::Str(_) => None,
        }
    }
}

/// Parses Valve's text KeyValues format: `"key" "value"` pairs and `"key" { ... }` blocks.
pub fn parse_vdf(text: &str) -> Option<Kv> {
    let mut chars = text.chars().peekable();
    let mut stack: Vec<(String, BTreeMap<String, Kv>)> = vec![(String::new(), BTreeMap::new())];
    let mut pending: Option<String> = None;
    let read_quoted = |chars: &mut std::iter::Peekable<std::str::Chars<'_>>| -> Option<String> {
        let mut s = String::new();
        loop {
            match chars.next()? {
                '"' => return Some(s),
                '\\' => match chars.next()? {
                    'n' => s.push('\n'),
                    't' => s.push('\t'),
                    c => s.push(c),
                },
                c => s.push(c),
            }
        }
    };
    while let Some(c) = chars.next() {
        match c {
            '"' => {
                let s = read_quoted(&mut chars)?;
                match pending.take() {
                    None => pending = Some(s),
                    Some(key) => {
                        stack.last_mut()?.1.insert(key, Kv::Str(s));
                    }
                }
            }
            '{' => {
                let key = pending.take()?;
                stack.push((key, BTreeMap::new()));
            }
            '}' => {
                let (key, map) = stack.pop()?;
                stack.last_mut()?.1.insert(key, Kv::Map(map));
            }
            '/' if chars.peek() == Some(&'/') => {
                for c in chars.by_ref() {
                    if c == '\n' {
                        break;
                    }
                }
            }
            _ => {}
        }
    }
    if stack.len() != 1 {
        return None;
    }
    Some(Kv::Map(stack.pop()?.1))
}

/// Steam library folders from `libraryfolders.vdf`.
pub fn steam_libraries(vdf: &str) -> Vec<String> {
    let Some(root) = parse_vdf(vdf) else {
        return Vec::new();
    };
    let Some(lf) = root.get("libraryfolders").and_then(Kv::map) else {
        return Vec::new();
    };
    lf.values()
        .filter_map(|v| v.str("path"))
        .map(|p| p.replace("\\\\", "\\"))
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SteamApp {
    pub app_id: String,
    pub name: String,
    pub install_dir: String,
    pub size_on_disk: u64,
}

/// One `appmanifest_<id>.acf`.
pub fn steam_app(acf: &str) -> Option<SteamApp> {
    let root = parse_vdf(acf)?;
    let s = root.get("AppState")?;
    Some(SteamApp {
        app_id: s.str("appid")?.to_string(),
        name: s.str("name").unwrap_or_default().to_string(),
        install_dir: s.str("installdir")?.to_string(),
        size_on_disk: s
            .str("SizeOnDisk")
            .and_then(|v| v.parse().ok())
            .unwrap_or(0),
    })
}

/// Library paths in an Xbox app `.GamingRoot` file: an 8-byte header, then UTF-16 paths
/// separated by NULs (relative to the drive, like `XboxGames`).
pub fn gaming_root_paths(bytes: &[u8]) -> Vec<String> {
    if bytes.len() < 10 || &bytes[0..4] != b"RGBX" {
        return Vec::new();
    }
    let (pairs, _) = bytes[8..].as_chunks::<2>();
    let units: Vec<u16> = pairs.iter().map(|c| u16::from_le_bytes(*c)).collect();
    units
        .split(|u| *u == 0)
        .map(String::from_utf16_lossy)
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/* ---------- Audit ---------- */

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Launcher {
    Steam,
    Epic,
    Xbox,
    Gog,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Severity {
    /// Allowed, but would fit better elsewhere.
    Info,
    Warning,
    Problem,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Reason {
    BigGameOnLightDrive,
    GameOnSystemDrive,
    GameOnDisposableDrive,
    SmallGameOnHeavyDrive,
    PersonalFilesOnSystemDrive,
    CacheOnSystemDrive,
    MaybeOnlyCopyOnDisposable,
}

/// How a finding is fixed. Only `CopyVerify` and `RelocateCache` are run by the app.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Method {
    /// The launcher's own "move install folder" (Steam, Xbox app): guided, never done behind its back.
    LauncherMove { launcher: Launcher },
    /// Launchers without a move feature (Epic, GOG): uninstall, then install to the target library.
    Reinstall { launcher: Launcher },
    /// Copy, verify (size + hash), then move the originals to their drive's Recycle Bin.
    CopyVerify,
    /// Point the tool to the new place with its environment variable, then copy-verify.
    RelocateCache { env_var: String },
}

impl Method {
    pub fn runs_in_app(&self) -> bool {
        matches!(self, Method::CopyVerify | Method::RelocateCache { .. })
    }
}

/// What the audit found that doesn't fit the policy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Finding {
    pub id: String,
    pub name: String,
    pub path: String,
    pub drive: String,
    pub bytes: u64,
    pub severity: Severity,
    pub reason: Reason,
    /// Drive letter it should go to, if any drive has the right role.
    pub target_drive: Option<String>,
    pub method: Method,
}

/// Where a game belongs, and how serious it is that it isn't there.
pub fn rate_game(policy: &Policy, drive_role: Role, size: u64) -> Option<(Reason, Severity, Role)> {
    let big = size >= policy.threshold_bytes();
    let wanted = if big {
        Role::HeavyGames
    } else {
        Role::LightGames
    };
    match drive_role {
        Role::System => Some((Reason::GameOnSystemDrive, Severity::Problem, wanted)),
        Role::Disposable => Some((Reason::GameOnDisposableDrive, Severity::Problem, wanted)),
        Role::LightGames if big => Some((
            Reason::BigGameOnLightDrive,
            Severity::Problem,
            Role::HeavyGames,
        )),
        Role::HeavyGames if !big => Some((
            Reason::SmallGameOnHeavyDrive,
            Severity::Info,
            Role::LightGames,
        )),
        _ => None,
    }
}

/// Personal files counted on the system drive: documents, media and archives.
pub const PERSONAL_EXTENSIONS: &[&str] = &[
    "pdf", "doc", "docx", "xls", "xlsx", "ppt", "pptx", "odt", "ods", "psd", "ai", "epub", "mp4",
    "mkv", "mov", "avi", "wmv", "webm", "m4v", "mp3", "flac", "wav", "m4a", "aac", "ogg", "jpg",
    "jpeg", "png", "heic", "tif", "tiff", "raw", "cr2", "nef", "arw", "dng", "zip", "7z", "rar",
    "iso", "tar", "gz", "xz", "bz2", "img",
];

pub const PERSONAL_MIN_BYTES: u64 = 100 * 1024 * 1024;
pub const CACHE_MIN_BYTES: u64 = 1024 * 1024 * 1024;

pub fn is_personal_file(name: &str) -> bool {
    std::path::Path::new(name)
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .is_some_and(|e| PERSONAL_EXTENSIONS.contains(&e.as_str()))
}

/// Tool caches in the user profile that the tool can be pointed elsewhere with an environment variable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CacheKind {
    pub id: &'static str,
    pub name: &'static str,
    /// Relative to the user profile.
    pub path: &'static str,
    pub env_var: &'static str,
    /// Programs that must be closed while it moves.
    pub processes: &'static [&'static str],
}

pub const CACHES: &[CacheKind] = &[
    CacheKind {
        id: "huggingface",
        name: "Hugging Face models",
        path: r".cache\huggingface",
        env_var: "HF_HOME",
        processes: &[],
    },
    CacheKind {
        id: "torch",
        name: "PyTorch hub",
        path: r".cache\torch",
        env_var: "TORCH_HOME",
        processes: &[],
    },
    CacheKind {
        id: "ollama",
        name: "Ollama models",
        path: r".ollama\models",
        env_var: "OLLAMA_MODELS",
        processes: &["ollama.exe", "ollama app.exe"],
    },
    CacheKind {
        id: "pip",
        name: "pip cache",
        path: r"AppData\Local\pip\Cache",
        env_var: "PIP_CACHE_DIR",
        processes: &["pip.exe", "python.exe"],
    },
    CacheKind {
        id: "npm",
        name: "npm cache",
        path: r"AppData\Local\npm-cache",
        env_var: "NPM_CONFIG_CACHE",
        processes: &["node.exe"],
    },
    CacheKind {
        id: "yarn",
        name: "Yarn cache",
        path: r"AppData\Local\Yarn\Cache",
        env_var: "YARN_CACHE_FOLDER",
        processes: &["node.exe"],
    },
    CacheKind {
        id: "nuget",
        name: "NuGet packages",
        path: r".nuget\packages",
        env_var: "NUGET_PACKAGES",
        processes: &["dotnet.exe", "devenv.exe", "msbuild.exe"],
    },
    CacheKind {
        id: "gradle",
        name: "Gradle",
        path: ".gradle",
        env_var: "GRADLE_USER_HOME",
        processes: &["java.exe", "studio64.exe"],
    },
    CacheKind {
        id: "conda",
        name: "Conda packages",
        path: r".conda\pkgs",
        env_var: "CONDA_PKGS_DIRS",
        processes: &["conda.exe", "python.exe"],
    },
    CacheKind {
        id: "playwright",
        name: "Playwright browsers",
        path: r"AppData\Local\ms-playwright",
        env_var: "PLAYWRIGHT_BROWSERS_PATH",
        processes: &["node.exe"],
    },
];

/// Drive layout for Smart Care: no misplaced items good; over 10 GB misplaced or the system
/// drive under 25 % free warning; system drive under 15 % free, possibly-unique data on the
/// disposable drive, or a drive whose health isn't OK critical.
pub fn layout_status(
    findings: &[Finding],
    system_free_percent: Option<f64>,
    any_drive_unhealthy: bool,
) -> Status {
    let unique_on_disposable = findings
        .iter()
        .any(|f| f.reason == Reason::MaybeOnlyCopyOnDisposable);
    let misplaced: u64 = findings
        .iter()
        .filter(|f| f.severity != Severity::Info)
        .map(|f| f.bytes)
        .sum();
    let free = system_free_percent.unwrap_or(100.0);
    if free < 15.0 || unique_on_disposable || any_drive_unhealthy {
        Status::Critical
    } else if misplaced > 10 * GB || free < 25.0 {
        Status::Warning
    } else {
        Status::Good
    }
}

/* ---------- Planning ---------- */

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CheckKind {
    TargetRole,
    FreeSpace,
    SourceHealth,
    TargetHealth,
    FilesInUse,
    ProgramsClosed,
    PluggedIn,
    TargetFree,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Check {
    pub kind: CheckKind,
    pub ok: bool,
    /// Short detail for the UI (program names, sizes...).
    pub detail: String,
}

/// What the Windows side measured for one step.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PreflightFacts {
    pub target_role_allows: bool,
    pub bytes: u64,
    pub target_free: u64,
    pub source_healthy: bool,
    pub target_healthy: bool,
    pub locked_by: Vec<String>,
    pub running: Vec<String>,
    pub on_battery: bool,
    pub target_exists: bool,
}

/// Target free space must be at least the size plus 10 %.
pub fn needed_space(bytes: u64) -> u64 {
    bytes + bytes / 10
}

pub fn preflight(f: &PreflightFacts) -> Vec<Check> {
    let check = |kind, ok: bool, detail: String| Check { kind, ok, detail };
    vec![
        check(CheckKind::TargetRole, f.target_role_allows, String::new()),
        check(
            CheckKind::FreeSpace,
            f.target_free >= needed_space(f.bytes),
            needed_space(f.bytes).to_string(),
        ),
        check(CheckKind::SourceHealth, f.source_healthy, String::new()),
        check(CheckKind::TargetHealth, f.target_healthy, String::new()),
        check(CheckKind::TargetFree, !f.target_exists, String::new()),
        check(
            CheckKind::FilesInUse,
            f.locked_by.is_empty(),
            f.locked_by.join(", "),
        ),
        check(
            CheckKind::ProgramsClosed,
            f.running.is_empty(),
            f.running.join(", "),
        ),
        check(CheckKind::PluggedIn, !f.on_battery, String::new()),
    ]
}

/// Rough copy time: the slower drive's speed (HDD about 120 MB/s, SATA SSD 400, NVMe 1500),
/// doubled for the verification read.
pub fn estimate_seconds(bytes: u64, source_mbps: u64, target_mbps: u64) -> u64 {
    let speed = source_mbps.min(target_mbps).max(1) * 1024 * 1024;
    (bytes.saturating_mul(2) / speed).max(1)
}

pub fn drive_speed_mbps(media_type: &str, bus_type: &str) -> u64 {
    if bus_type.eq_ignore_ascii_case("NVMe") {
        1500
    } else if media_type.eq_ignore_ascii_case("SSD") {
        400
    } else {
        120
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VDF: &str = r#"
"libraryfolders"
{
	"0"
	{
		"path"		"C:\\Program Files (x86)\\Steam"
		"apps" { "228980" "267921442" }
	}
	"1"
	{
		"path"		"G:\\Games\\Steam"
		// comment
		"apps" { "255710" "17128372956" }
	}
}"#;

    const ACF: &str = r#""AppState"
{
	"appid"		"1184370"
	"name"		"Pathfinder: Wrath of the Righteous"
	"installdir"		"Pathfinder Second Adventure"
	"SizeOnDisk"		"34218742955"
}"#;

    #[test]
    fn default_policy_is_valid() {
        let p = Policy::default_policy();
        assert!(p.validate().is_ok());
        assert_eq!(p.role_of("c:"), Role::System);
        assert_eq!(p.role_of("Q:"), Role::Unassigned);
        assert_eq!(p.drive_for(Role::Disposable), Some("S:"));
        assert_eq!(p.threshold_bytes(), 50 * GB);
        let mut bad = p.clone();
        bad.drives.push(DriveRule {
            letter: "c:".into(),
            role: Role::LightGames,
            volume_guid: None,
            disk_serial: None,
        });
        assert!(bad.validate().is_err());
    }

    fn seen(letter: &str, guid: &str) -> SeenVolume {
        SeenVolume {
            letter: letter.into(),
            volume_guid: Some(guid.into()),
            disk_serial: Some(format!("SN-{guid}")),
        }
    }

    #[test]
    fn binds_by_guid_and_follows_letter_changes() {
        let p = Policy::default_policy();
        let now = [seen("C:", "c"), seen("X:", "x"), seen("G:", "g")];
        let (bound, notices) = bind_policy(&p, &now);
        assert_eq!(bound.drives[0].volume_guid.as_deref(), Some("c"));
        assert_eq!(bound.drives[1].disk_serial.as_deref(), Some("SN-x"));
        assert_eq!(
            notices,
            vec![PolicyNotice::Missing {
                letter: "S:".into()
            }]
        );

        // The heavy-games drive comes back as Y: and a new drive takes X:.
        let later = [
            seen("C:", "c"),
            seen("Y:", "x"),
            seen("X:", "new"),
            seen("G:", "g"),
        ];
        let (moved, notices) = bind_policy(&bound, &later);
        assert_eq!(moved.role_of("Y:"), Role::HeavyGames);
        assert_eq!(
            moved.role_of("X:"),
            Role::Unassigned,
            "a different drive on the old letter gets no role"
        );
        assert!(notices.contains(&PolicyNotice::LetterChanged {
            from: "X:".into(),
            to: "Y:".into()
        }));
    }

    #[test]
    fn parses_steam_files() {
        assert_eq!(
            steam_libraries(VDF),
            vec![
                r"C:\Program Files (x86)\Steam".to_string(),
                r"G:\Games\Steam".to_string()
            ]
        );
        let app = steam_app(ACF).unwrap();
        assert_eq!(app.name, "Pathfinder: Wrath of the Righteous");
        assert_eq!(app.install_dir, "Pathfinder Second Adventure");
        assert_eq!(app.size_on_disk, 34_218_742_955);
        assert!(
            parse_vdf(r#""a" { "b" "c" "#).is_none(),
            "unbalanced braces"
        );
        assert!(steam_app("garbage").is_none());
    }

    #[test]
    fn parses_gaming_root() {
        let mut bytes = b"RGBX\x01\x00\x00\x00".to_vec();
        for u in "XboxGames".encode_utf16().chain([0u16]) {
            bytes.extend_from_slice(&u.to_le_bytes());
        }
        assert_eq!(gaming_root_paths(&bytes), vec!["XboxGames".to_string()]);
        assert!(gaming_root_paths(b"nope").is_empty());
    }

    #[test]
    fn rates_games_by_drive_and_size() {
        let p = Policy::default_policy();
        let big = 60 * GB;
        let small = 10 * GB;
        assert_eq!(
            rate_game(&p, Role::LightGames, big),
            Some((
                Reason::BigGameOnLightDrive,
                Severity::Problem,
                Role::HeavyGames
            ))
        );
        assert_eq!(rate_game(&p, Role::LightGames, small), None);
        assert_eq!(rate_game(&p, Role::HeavyGames, big), None);
        assert_eq!(
            rate_game(&p, Role::HeavyGames, small),
            Some((
                Reason::SmallGameOnHeavyDrive,
                Severity::Info,
                Role::LightGames
            ))
        );
        assert_eq!(
            rate_game(&p, Role::System, small).map(|r| r.2),
            Some(Role::LightGames)
        );
        assert_eq!(
            rate_game(&p, Role::Disposable, big).map(|r| r.0),
            Some(Reason::GameOnDisposableDrive)
        );
    }

    fn finding(reason: Reason, severity: Severity, gb: u64) -> Finding {
        Finding {
            id: "x".into(),
            name: "x".into(),
            path: "x".into(),
            drive: "C:".into(),
            bytes: gb * GB,
            severity,
            reason,
            target_drive: None,
            method: Method::CopyVerify,
        }
    }

    #[test]
    fn layout_thresholds() {
        assert_eq!(layout_status(&[], Some(40.0), false), Status::Good);
        assert_eq!(
            layout_status(
                &[finding(Reason::SmallGameOnHeavyDrive, Severity::Info, 40)],
                Some(40.0),
                false
            ),
            Status::Good,
            "info only"
        );
        assert_eq!(
            layout_status(
                &[finding(Reason::BigGameOnLightDrive, Severity::Problem, 11)],
                Some(40.0),
                false
            ),
            Status::Warning
        );
        assert_eq!(layout_status(&[], Some(20.0), false), Status::Warning);
        assert_eq!(layout_status(&[], Some(13.0), false), Status::Critical);
        assert_eq!(
            layout_status(
                &[finding(
                    Reason::MaybeOnlyCopyOnDisposable,
                    Severity::Problem,
                    0
                )],
                Some(40.0),
                false
            ),
            Status::Critical
        );
        assert_eq!(layout_status(&[], Some(40.0), true), Status::Critical);
    }

    #[test]
    fn preflight_and_estimates() {
        let ok = PreflightFacts {
            target_role_allows: true,
            bytes: 100,
            target_free: 110,
            source_healthy: true,
            target_healthy: true,
            ..Default::default()
        };
        assert!(preflight(&ok).iter().all(|c| c.ok));
        let tight = PreflightFacts {
            target_free: 109,
            ..ok.clone()
        };
        assert!(
            !preflight(&tight)
                .iter()
                .find(|c| c.kind == CheckKind::FreeSpace)
                .unwrap()
                .ok
        );
        let locked = PreflightFacts {
            locked_by: vec!["Steam".into()],
            on_battery: true,
            ..ok
        };
        let checks = preflight(&locked);
        assert_eq!(checks.iter().filter(|c| !c.ok).count(), 2);
        assert_eq!(estimate_seconds(120 * 1024 * 1024, 120, 1500), 2);
        assert_eq!(drive_speed_mbps("SSD", "NVMe"), 1500);
        assert_eq!(drive_speed_mbps("HDD", "SATA"), 120);
        assert!(is_personal_file("Holiday.MP4") && !is_personal_file("game.pak"));
        assert!(
            Method::CopyVerify.runs_in_app()
                && !Method::LauncherMove {
                    launcher: Launcher::Steam
                }
                .runs_in_app()
        );
    }
}
