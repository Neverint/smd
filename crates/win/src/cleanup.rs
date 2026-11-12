//! Cleanup: scan the rules read-only, then remove only what the user kept selected.

use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::Serialize;
use smd_core::cleanup::{Group, ItemLevel, Kind, Rule, Safety, expand_vars, glob_match};
use smd_core::safety;

use crate::walk::measure;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Item {
    pub path: String,
    /// Short name shown in the review list.
    pub name: String,
    pub bytes: u64,
    pub files: u64,
    pub modified_ms: Option<i64>,
    pub is_dir: bool,
    /// The rule root this item was found under (checked again before removal).
    #[serde(skip)]
    pub root: String,
    #[serde(skip)]
    pub item_is_root: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Category {
    pub id: String,
    pub group: Group,
    pub safety: Safety,
    pub admin: bool,
    pub permanent: bool,
    pub default_selected: bool,
    pub bytes: u64,
    pub files: u64,
    pub items: Vec<Item>,
    /// Items skipped because they match an exclusion in Settings.
    pub excluded: u64,
    /// Programs from the rule's list that are running now (their files may be in use).
    pub running: Vec<String>,
    /// A folder could not be read (usually needs admin): the size is a lower bound.
    pub unreadable: bool,
}

pub struct ScanEnv<'a> {
    pub var: &'a (dyn Fn(&str) -> Option<String> + Sync),
    pub exclusions: &'a [String],
    pub running: &'a HashSet<String>,
    pub now_ms: i64,
}

/// Expands a rule path and resolves `*` segments against existing folders.
pub fn resolve_roots(template: &str, var: &dyn Fn(&str) -> Option<String>) -> Vec<PathBuf> {
    let Some(expanded) = expand_vars(template, var) else {
        return Vec::new();
    };
    let mut current: Vec<PathBuf> = Vec::new();
    for (i, seg) in expanded
        .split(['\\', '/'])
        .filter(|s| !s.is_empty())
        .enumerate()
    {
        if i == 0 {
            // Drive (`C:`): keep the separator so the path is absolute.
            current.push(PathBuf::from(format!("{seg}\\")));
            continue;
        }
        if seg.contains(['*', '?']) {
            let mut next = Vec::new();
            for base in &current {
                let Ok(entries) = fs::read_dir(base) else {
                    continue;
                };
                for e in entries.flatten() {
                    let is_real_dir = e.file_type().is_ok_and(|t| t.is_dir() && !t.is_symlink());
                    if is_real_dir && glob_match(seg, &e.file_name().to_string_lossy()) {
                        next.push(e.path());
                    }
                }
            }
            current = next;
        } else {
            for p in &mut current {
                p.push(seg);
            }
        }
    }
    current.sort();
    current
        .into_iter()
        .filter(|p| fs::symlink_metadata(p).is_ok_and(|m| !m.file_type().is_symlink()))
        .collect()
}

fn is_excluded(path: &str, exclusions: &[String]) -> bool {
    exclusions
        .iter()
        .any(|e| safety::same_path(path, e) || safety::is_inside(path, e))
}

/// Display name of a root-level item: the part after the static prefix (the profile folder onward).
fn root_item_name(template: &str, path: &Path, var: &dyn Fn(&str) -> Option<String>) -> String {
    let full = path.to_string_lossy();
    if let Some(star) = template.find('*') {
        let prefix_template = template[..star].trim_end_matches('\\');
        if let Some(prefix) = expand_vars(prefix_template, var)
            && full.len() > prefix.len() + 1
            && full[..prefix.len()].eq_ignore_ascii_case(&prefix)
        {
            return full[prefix.len() + 1..].to_string();
        }
    }
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| full.into_owned())
}

/// Scans one rule. Read-only.
pub fn scan_rule(rule: &Rule, env: &ScanEnv<'_>) -> Category {
    let mut cat = Category {
        id: rule.id.clone(),
        group: rule.group,
        safety: rule.safety,
        admin: rule.admin,
        permanent: rule.permanent,
        default_selected: rule.default_selected && !rule.admin,
        bytes: 0,
        files: 0,
        items: Vec::new(),
        excluded: 0,
        running: rule
            .processes
            .iter()
            .filter(|p| env.running.contains(&p.to_lowercase()))
            .cloned()
            .collect(),
        unreadable: false,
    };

    if rule.kind == Kind::RecycleBin {
        match crate::system::recycle_bin_size() {
            Ok((bytes, items)) => {
                cat.bytes = bytes;
                cat.files = items;
            }
            Err(_) => cat.unreadable = true,
        }
        return cat;
    }

    let min_age_ms = i64::from(rule.min_age_hours) * 3_600_000;
    for template in &rule.paths {
        for root in resolve_roots(template, env.var) {
            let root_str = root.to_string_lossy().into_owned();
            let Ok(meta) = fs::symlink_metadata(&root) else {
                continue;
            };
            let mut candidates: Vec<(PathBuf, String, bool)> = Vec::new();
            if !meta.is_dir() || rule.item_level == ItemLevel::Root {
                let name = root_item_name(template, &root, env.var);
                candidates.push((root.clone(), name, true));
            } else {
                match fs::read_dir(&root) {
                    Ok(entries) => {
                        for e in entries.flatten() {
                            if e.file_type().is_ok_and(|t| t.is_symlink()) {
                                continue;
                            }
                            let name = e.file_name().to_string_lossy().into_owned();
                            if !rule.include.is_empty()
                                && !rule.include.iter().any(|p| glob_match(p, &name))
                            {
                                continue;
                            }
                            candidates.push((e.path(), name, false));
                        }
                    }
                    Err(_) => cat.unreadable = true,
                }
            }

            for (path, name, item_is_root) in candidates {
                let path_str = path.to_string_lossy().into_owned();
                if is_excluded(&path_str, env.exclusions) {
                    cat.excluded += 1;
                    continue;
                }
                if safety::check_removal(&path_str, &root_str, item_is_root).is_err() {
                    log::warn!(
                        "cleanup rule {} matched a protected path, skipped: {path_str}",
                        rule.id
                    );
                    continue;
                }
                let m = measure(&path);
                if m.unreadable && m.bytes == 0 {
                    cat.unreadable = true;
                    continue;
                }
                if min_age_ms > 0 && m.newest_ms.is_some_and(|t| t > env.now_ms - min_age_ms) {
                    continue;
                }
                if m.bytes == 0 && m.files == 0 && item_is_root {
                    continue;
                }
                cat.bytes += m.bytes;
                cat.files += m.files;
                cat.items.push(Item {
                    is_dir: fs::symlink_metadata(&path).is_ok_and(|x| x.is_dir()),
                    path: path_str,
                    name,
                    bytes: m.bytes,
                    files: m.files,
                    modified_ms: m.newest_ms,
                    root: root_str.clone(),
                    item_is_root,
                });
            }
        }
    }
    cat.items
        .sort_by(|a, b| b.bytes.cmp(&a.bytes).then_with(|| a.path.cmp(&b.path)));
    cat
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, serde::Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum DeleteMode {
    #[default]
    RecycleBin,
    Permanent,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Failure {
    pub path: String,
    pub reason: FailReason,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum FailReason {
    /// Open in another program, or Windows refused access.
    InUse,
    /// Already gone before we got to it.
    Gone,
    /// Failed a safety check (should never happen with the shipped rules).
    Refused,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Outcome {
    /// Removed paths with their size.
    pub removed: Vec<(String, u64)>,
    pub failed: Vec<Failure>,
}

const RECYCLE_BATCH: usize = 100;

/// Where removed items go.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Destination {
    RecycleBin,
    Permanent,
    /// `S:\Quarantine` (items already on that drive go to its Recycle Bin instead).
    Quarantine {
        root: std::path::PathBuf,
        batch_id: u64,
        now_ms: i64,
    },
}

/// Like `remove_items`, with quarantine. Returns the quarantine manifest when one was written.
pub fn remove_items_to(
    items: &[Item],
    dest: &Destination,
) -> (Outcome, Option<std::path::PathBuf>) {
    match dest {
        Destination::RecycleBin => (remove_items(items, DeleteMode::RecycleBin), None),
        Destination::Permanent => (remove_items(items, DeleteMode::Permanent), None),
        Destination::Quarantine {
            root,
            batch_id,
            now_ms,
        } => {
            let drive = |p: &str| p.get(0..2).map(|s| s.to_ascii_uppercase());
            let qdrive = drive(&root.to_string_lossy());
            let (local, away): (Vec<Item>, Vec<Item>) = items
                .iter()
                .cloned()
                .partition(|i| drive(&i.path) == qdrive);
            let mut out = remove_items(&local, DeleteMode::RecycleBin);
            let (checked, ready) = check_all(&away);
            out.failed.extend(checked.failed);
            let (q, manifest) = crate::quarantine::put(root, *batch_id, *now_ms, &ready);
            out.removed.extend(q.removed);
            out.failed.extend(q.failed);
            (out, manifest)
        }
    }
}

/// The checks every removal goes through: safety rules, no links, still there.
fn check_all(items: &[Item]) -> (Outcome, Vec<&Item>) {
    let mut out = Outcome::default();
    let mut ready = Vec::new();
    for item in items {
        if safety::check_removal(&item.path, &item.root, item.item_is_root).is_err() {
            out.failed.push(Failure {
                path: item.path.clone(),
                reason: FailReason::Refused,
            });
            continue;
        }
        match fs::symlink_metadata(&item.path) {
            Ok(m) if m.file_type().is_symlink() => out.failed.push(Failure {
                path: item.path.clone(),
                reason: FailReason::Refused,
            }),
            Ok(_) => ready.push(item),
            Err(_) => out.failed.push(Failure {
                path: item.path.clone(),
                reason: FailReason::Gone,
            }),
        }
    }
    (out, ready)
}

/// Removes items after checking each one again. Recycle Bin by default.
pub fn remove_items(items: &[Item], mode: DeleteMode) -> Outcome {
    let mut out = Outcome::default();
    let mut ready: Vec<&Item> = Vec::new();
    for item in items {
        if safety::check_removal(&item.path, &item.root, item.item_is_root).is_err() {
            out.failed.push(Failure {
                path: item.path.clone(),
                reason: FailReason::Refused,
            });
            continue;
        }
        match fs::symlink_metadata(&item.path) {
            Ok(m) if m.file_type().is_symlink() => {
                out.failed.push(Failure {
                    path: item.path.clone(),
                    reason: FailReason::Refused,
                });
            }
            Ok(_) => ready.push(item),
            Err(_) => out.failed.push(Failure {
                path: item.path.clone(),
                reason: FailReason::Gone,
            }),
        }
    }

    match mode {
        DeleteMode::Permanent => {
            for item in ready {
                let p = Path::new(&item.path);
                let r = if item.is_dir {
                    fs::remove_dir_all(p)
                } else {
                    fs::remove_file(p)
                };
                match r {
                    Ok(()) => out.removed.push((item.path.clone(), item.bytes)),
                    Err(_) => out.failed.push(Failure {
                        path: item.path.clone(),
                        reason: FailReason::InUse,
                    }),
                }
            }
        }
        DeleteMode::RecycleBin => {
            for chunk in ready.chunks(RECYCLE_BATCH) {
                // One shell operation per batch; items the shell skips (in use) stay on disk.
                if let Err(e) = trash::delete_all(chunk.iter().map(|i| &i.path)) {
                    log::info!("recycle batch reported skipped items: {e}");
                }
                for item in chunk {
                    if Path::new(&item.path).exists() {
                        // Retry alone once: a batch can stop on the first unreadable path.
                        let _ = trash::delete(&item.path);
                    }
                    if Path::new(&item.path).exists() {
                        out.failed.push(Failure {
                            path: item.path.clone(),
                            reason: FailReason::InUse,
                        });
                    } else {
                        out.removed.push((item.path.clone(), item.bytes));
                    }
                }
            }
        }
    }
    out
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreReport {
    pub restored: u64,
    /// No longer in the Recycle Bin (it was emptied).
    pub missing: u64,
    /// A file with the same name is back in place, or Windows refused.
    pub failed: u64,
}

/// Puts items back from the Recycle Bin. Only items deleted after `since_unix_s` are considered.
pub fn restore(paths: &[String], since_unix_s: i64) -> io::Result<RestoreReport> {
    let wanted: HashSet<String> = paths.iter().map(|p| safety::normalize(p)).collect();
    let mut best: std::collections::HashMap<String, trash::TrashItem> =
        std::collections::HashMap::new();
    for item in trash::os_limited::list().map_err(io::Error::other)? {
        if item.time_deleted < since_unix_s - 60 {
            continue;
        }
        let key = safety::normalize(&item.original_path().to_string_lossy());
        if !wanted.contains(&key) {
            continue;
        }
        let newer = best
            .get(&key)
            .is_none_or(|b| item.time_deleted > b.time_deleted);
        if newer {
            best.insert(key, item);
        }
    }
    let mut report = RestoreReport {
        missing: (wanted.len() - best.len()) as u64,
        ..Default::default()
    };
    for item in best.into_values() {
        match trash::os_limited::restore_all([item]) {
            Ok(()) => report.restored += 1,
            Err(_) => report.failed += 1,
        }
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write(path: &Path, bytes: usize) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::File::create(path)
            .unwrap()
            .write_all(&vec![1u8; bytes])
            .unwrap();
    }

    fn rule(json: &str) -> Rule {
        serde_json::from_str(json).unwrap()
    }

    fn env_for<'a>(root: &'a Path) -> impl Fn(&str) -> Option<String> + 'a {
        move |k: &str| (k == "ROOT").then(|| root.to_string_lossy().into_owned())
    }

    #[test]
    fn resolves_wildcard_segments() {
        let dir = tempfile::tempdir().unwrap();
        write(
            &dir.path().join("User Data/Default/Cache/Cache_Data/f_1"),
            10,
        );
        write(
            &dir.path().join("User Data/Profile 1/Cache/Cache_Data/f_1"),
            10,
        );
        fs::create_dir_all(dir.path().join("User Data/Empty")).unwrap();
        let var = env_for(dir.path());
        let roots = resolve_roots(r"{ROOT}\User Data\*\Cache\Cache_Data", &var);
        assert_eq!(roots.len(), 2);
        assert!(
            roots[0].ends_with("Default/Cache/Cache_Data")
                || roots[0].ends_with(r"Default\Cache\Cache_Data")
        );
        assert!(resolve_roots(r"{NOPE}\x", &var).is_empty());
    }

    #[test]
    fn scans_children_with_age_include_and_exclusions() {
        let dir = tempfile::tempdir().unwrap();
        let temp = dir.path().join("Temp");
        write(&temp.join("old.tmp"), 100);
        write(&temp.join("keep/me.txt"), 50);
        write(&temp.join("skip.log"), 70);
        let var = env_for(dir.path());
        let running = HashSet::from(["explorer.exe".to_string()]);
        let exclusions = vec![temp.join("keep").to_string_lossy().into_owned()];
        let r = rule(
            r#"{"id":"t","group":"system","kind":"files","paths":["{ROOT}\\Temp"],"include":["*.tmp","keep"],"safety":"safe","defaultSelected":true,"processes":["explorer.exe"]}"#,
        );
        let env = ScanEnv {
            var: &var,
            exclusions: &exclusions,
            running: &running,
            now_ms: i64::MAX / 2,
        };
        let cat = scan_rule(&r, &env);
        assert_eq!(cat.items.len(), 1);
        assert_eq!(cat.items[0].name, "old.tmp");
        assert_eq!(cat.bytes, 100);
        assert_eq!(cat.excluded, 1);
        assert_eq!(cat.running, vec!["explorer.exe".to_string()]);
        assert!(cat.default_selected);

        // Everything is newer than 24 h, so an age rule finds nothing.
        let aged = rule(
            r#"{"id":"t","group":"system","kind":"files","paths":["{ROOT}\\Temp"],"minAgeHours":24,"safety":"safe"}"#,
        );
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;
        let env = ScanEnv {
            var: &var,
            exclusions: &[],
            running: &running,
            now_ms: now,
        };
        assert!(scan_rule(&aged, &env).items.is_empty());
    }

    #[test]
    fn root_level_items_and_admin_rules() {
        let dir = tempfile::tempdir().unwrap();
        write(&dir.path().join("UD/Default/GPUCache/data_0"), 30);
        let var = env_for(dir.path());
        let none = HashSet::new();
        let env = ScanEnv {
            var: &var,
            exclusions: &[],
            running: &none,
            now_ms: 0,
        };
        let r = rule(
            r#"{"id":"b","group":"browsers","kind":"files","paths":["{ROOT}\\UD\\*\\GPUCache"],"itemLevel":"root","safety":"safe","defaultSelected":true,"admin":true}"#,
        );
        let cat = scan_rule(&r, &env);
        assert_eq!(cat.items.len(), 1);
        assert!(
            cat.items[0].name == r"Default\GPUCache" || cat.items[0].name == "Default/GPUCache",
            "{}",
            cat.items[0].name
        );
        assert!(cat.items[0].item_is_root);
        assert!(!cat.default_selected, "admin rules are never pre-selected");
    }

    /// Uses the real Recycle Bin with a file this test creates, then puts it back.
    #[cfg(windows)]
    #[test]
    fn recycles_and_restores_own_test_file() {
        if std::env::var_os("CI").is_some() {
            // CI runners have no interactive shell session; run it on a real PC.
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("Temp");
        write(&root.join("smd-recycle-test.tmp"), 64);
        let var = env_for(dir.path());
        let none = HashSet::new();
        let env = ScanEnv {
            var: &var,
            exclusions: &[],
            running: &none,
            now_ms: 0,
        };
        let r = rule(
            r#"{"id":"t","group":"system","kind":"files","paths":["{ROOT}\\Temp"],"safety":"safe"}"#,
        );
        let items = scan_rule(&r, &env).items;
        let started = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let out = remove_items(&items, DeleteMode::RecycleBin);
        assert_eq!(out.removed.len(), 1, "{:?}", out.failed);
        assert!(!Path::new(&items[0].path).exists());

        let report = restore(&[items[0].path.clone()], started).unwrap();
        assert_eq!(
            report,
            RestoreReport {
                restored: 1,
                missing: 0,
                failed: 0
            }
        );
        assert!(
            Path::new(&items[0].path).exists(),
            "the file is back in place"
        );
    }

    #[test]
    fn permanent_removal_rechecks_safety() {
        let dir = tempfile::tempdir().unwrap();
        let temp = dir.path().join("Temp");
        write(&temp.join("a.tmp"), 10);
        write(&temp.join("sub/b.tmp"), 20);
        let var = env_for(dir.path());
        let none = HashSet::new();
        let env = ScanEnv {
            var: &var,
            exclusions: &[],
            running: &none,
            now_ms: 0,
        };
        let r = rule(
            r#"{"id":"t","group":"system","kind":"files","paths":["{ROOT}\\Temp"],"safety":"safe"}"#,
        );
        let mut items = scan_rule(&r, &env).items;
        assert_eq!(items.len(), 2);
        // A tampered item outside its root is refused.
        let mut evil = items[0].clone();
        evil.path = dir.path().join("other").to_string_lossy().into_owned();
        items.push(evil);
        let out = remove_items(&items, DeleteMode::Permanent);
        assert_eq!(out.removed.len(), 2);
        assert_eq!(out.removed.iter().map(|r| r.1).sum::<u64>(), 30);
        assert_eq!(
            out.failed,
            vec![Failure {
                path: items[2].path.clone(),
                reason: FailReason::Refused
            }]
        );
        assert!(temp.exists(), "the root folder itself stays");
        assert_eq!(fs::read_dir(&temp).unwrap().count(), 0);
    }
}
