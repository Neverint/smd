//! Installed apps: name matching for the leftovers review.
//!
//! The rules are strict on purpose: a folder is suggested only when its name is the
//! app's own name, never a shared or generic one, and never anything from Microsoft.

use serde::{Deserialize, Serialize};

/// How sure we are that a leftover belongs to the uninstalled app.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Confidence {
    Low,
    Medium,
    /// The app's own install folder, still on disk.
    High,
}

/// Folder names that many apps share or that Windows owns. Never suggested.
const GENERIC: &[&str] = &[
    "common",
    "common files",
    "microsoft",
    "windows",
    "temp",
    "tmp",
    "cache",
    "caches",
    "programs",
    "packages",
    "data",
    "logs",
    "log",
    "config",
    "settings",
    "updates",
    "update",
    "installer",
    "app",
    "apps",
    "bin",
    "lib",
    "local",
    "roaming",
    "user data",
    "users",
    "default",
    "shared",
    "tools",
    "plugins",
    "google",
    "mozilla",
    "nvidia",
    "nvidia corporation",
    "intel",
    "amd",
    "apple",
    "adobe",
    "steam",
    "epic games",
    "python",
    "java",
    "oracle",
    "npm",
    "node",
    "electron",
    "crashpad",
    "crashdumps",
    "d3dscache",
    "fonts",
    "virtualstore",
    "connecteddevicesplatform",
];

/// Lower-case, without version numbers, architecture tags or bracketed parts:
/// "Mozilla Firefox (x64 en-US) 128.0" → "mozilla firefox".
pub fn normalize_name(name: &str) -> String {
    let mut s = String::with_capacity(name.len());
    let mut depth = 0;
    for c in name.chars() {
        match c {
            '(' | '[' => depth += 1,
            ')' | ']' => depth = (depth - 1).max(0),
            _ if depth == 0 => s.push(c),
            _ => {}
        }
    }
    let drop = [
        "x64", "x86", "64-bit", "32-bit", "(64-bit)", "amd64", "arm64", "version", "v",
    ];
    let words: Vec<String> = s
        .to_lowercase()
        .split(|c: char| c.is_whitespace() || c == '_')
        .map(|w| w.trim_matches(|c: char| c == '-' || c == ',' || c == '.'))
        .filter(|w| !w.is_empty())
        .filter(|w| !drop.contains(w))
        // Version-like words: 1.2.3, v12, 2024
        .filter(|w| {
            !w.trim_start_matches('v')
                .chars()
                .all(|c| c.is_ascii_digit() || c == '.')
        })
        .map(str::to_string)
        .collect();
    words.join(" ")
}

/// Publisher without legal suffixes: "Mozilla Corporation" → "mozilla".
pub fn normalize_publisher(publisher: &str) -> String {
    let suffixes = [
        "corporation",
        "corp",
        "inc",
        "ltd",
        "llc",
        "gmbh",
        "s.a.",
        "sa",
        "limited",
        "co",
        "company",
        "ag",
        "bv",
        "srl",
    ];
    publisher
        .to_lowercase()
        .split(|c: char| c.is_whitespace() || c == ',' || c == '.')
        .filter(|w| !w.is_empty() && !suffixes.contains(w))
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn is_microsoft(publisher: &str) -> bool {
    publisher.to_lowercase().contains("microsoft")
}

/// True when a folder name is the app's name (spaces optional): "Notepad++" ↔ "notepad++",
/// "VideoLAN" is not "VLC media player".
pub fn folder_matches_app(folder: &str, app_name: &str) -> bool {
    let f = folder.to_lowercase();
    if f.len() < 3 || GENERIC.contains(&f.as_str()) {
        return false;
    }
    let app = normalize_name(app_name);
    if app.len() < 3 || GENERIC.contains(&app.as_str()) {
        return false;
    }
    let squash = |s: &str| {
        s.chars()
            .filter(|c| !c.is_whitespace() && *c != '-' && *c != '_')
            .collect::<String>()
    };
    f == app || squash(&f) == squash(&app)
}

/// True when a folder name is the publisher's name (for `<Publisher>\<App>` paths).
pub fn folder_matches_publisher(folder: &str, publisher: &str) -> bool {
    let f = folder.to_lowercase();
    if is_microsoft(publisher) || GENERIC.contains(&f.as_str()) {
        return false;
    }
    let p = normalize_publisher(publisher);
    p.len() >= 3 && (f == p || f == publisher.to_lowercase())
}

/// `YYYYMMDD` from the registry → (year, month, day).
pub fn parse_install_date(s: &str) -> Option<(u16, u8, u8)> {
    let s = s.trim();
    if s.len() != 8 || !s.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let y: u16 = s[0..4].parse().ok()?;
    let m: u8 = s[4..6].parse().ok()?;
    let d: u8 = s[6..8].parse().ok()?;
    ((1990..=2100).contains(&y) && (1..=12).contains(&m) && (1..=31).contains(&d))
        .then_some((y, m, d))
}

/// A winget package id we are willing to pass to the WinGet module.
pub fn is_safe_package_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 128
        && id.chars().next().is_some_and(|c| c.is_ascii_alphanumeric())
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | '+'))
}

/// An Appx package full name (Name_Version_Arch_ResourceId_PublisherId).
pub fn is_safe_appx_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 255
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | '~'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_names() {
        assert_eq!(
            normalize_name("Mozilla Firefox (x64 en-US) 128.0.3"),
            "mozilla firefox"
        );
        assert_eq!(normalize_name("7-Zip 24.08 (x64)"), "7-zip");
        assert_eq!(normalize_name("Notepad++ (64-bit x64)"), "notepad++");
        assert_eq!(normalize_name("Discord"), "discord");
        assert_eq!(normalize_name("Python 3.12.4 (64-bit)"), "python");
        assert_eq!(normalize_publisher("Mozilla Corporation"), "mozilla");
        assert_eq!(normalize_publisher("Valve Corp."), "valve");
    }

    #[test]
    fn matches_only_own_folders() {
        assert!(folder_matches_app("Discord", "Discord"));
        assert!(folder_matches_app("Notepad++", "Notepad++ (64-bit x64)"));
        assert!(folder_matches_app("OBS-Studio", "OBS Studio"));
        assert!(!folder_matches_app("VideoLAN", "VLC media player"));
        assert!(!folder_matches_app("Common Files", "Common Files"));
        assert!(
            !folder_matches_app("Python", "Python 3.12.4 (64-bit)"),
            "generic runtime names are never matched"
        );
        assert!(!folder_matches_app("ab", "ab"));
        assert!(
            !folder_matches_publisher("Mozilla", "Mozilla Corporation"),
            "generic publisher folders are skipped"
        );
        assert!(folder_matches_publisher("Valve", "Valve Corp."));
        assert!(!folder_matches_publisher(
            "Microsoft",
            "Microsoft Corporation"
        ));
    }

    #[test]
    fn dates_and_ids() {
        assert_eq!(parse_install_date("20241031"), Some((2024, 10, 31)));
        assert_eq!(parse_install_date("2024-10-31"), None);
        assert_eq!(parse_install_date("19801010"), None);
        assert!(is_safe_package_id("Mozilla.Firefox"));
        assert!(is_safe_package_id("Notepad++.Notepad++"));
        assert!(!is_safe_package_id("x; Remove-Item C:\\"));
        assert!(!is_safe_package_id("-Force"));
        assert!(is_safe_appx_name(
            "Microsoft.WindowsCalculator_11.2410.0.0_x64__8wekyb3d8bbwe"
        ));
        assert!(!is_safe_appx_name("a b"));
    }
}
