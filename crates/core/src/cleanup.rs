//! Declarative cleanup rules (`rules/cleanup.json`).
//!
//! A rule names one or more root paths with `{VARIABLE}` placeholders and `*`
//! wildcard segments. The scanner (crate `smd-win`) expands them, lists the
//! items and measures them; nothing here touches the file system.

use serde::{Deserialize, Serialize};

/// The rules shipped with the app.
pub const BUILTIN_RULES_JSON: &str = include_str!("../../../rules/cleanup.json");

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Group {
    System,
    Browsers,
    RecycleBin,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Kind {
    /// Files and folders under the rule's paths.
    Files,
    /// The Windows Recycle Bin, emptied through the shell.
    RecycleBin,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Safety {
    /// Windows or the app recreates it as needed.
    Safe,
    /// Safe, but you lose something you might want (old crash data, the Recycle Bin).
    Advanced,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum ItemLevel {
    /// Each entry directly inside the path is one item. The path itself stays.
    #[default]
    Children,
    /// The path itself is one item (a cache folder the app recreates).
    Root,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Rule {
    pub id: String,
    pub group: Group,
    pub kind: Kind,
    #[serde(default)]
    pub paths: Vec<String>,
    #[serde(default)]
    pub item_level: ItemLevel,
    /// File-name patterns (`*`, `?`) an item must match. Empty = every item.
    #[serde(default)]
    pub include: Vec<String>,
    /// Only items whose newest file is at least this old. 0 = no age limit.
    #[serde(default)]
    pub min_age_hours: u32,
    /// Needs the elevated helper to delete (Phase 4).
    #[serde(default)]
    pub admin: bool,
    pub safety: Safety,
    #[serde(default)]
    pub default_selected: bool,
    /// Programs that keep these files open (lower-case exe names).
    #[serde(default)]
    pub processes: Vec<String>,
    /// The action cannot be undone (emptying the Recycle Bin).
    #[serde(default)]
    pub permanent: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct RuleFile {
    version: u32,
    rules: Vec<Rule>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum RuleError {
    Parse(String),
    UnsupportedVersion(u32),
    DuplicateId(String),
    Invalid(String),
}

impl std::fmt::Display for RuleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RuleError::Parse(e) => write!(f, "rules file is not valid: {e}"),
            RuleError::UnsupportedVersion(v) => {
                write!(f, "rules file version {v} is not supported")
            }
            RuleError::DuplicateId(id) => write!(f, "rule id {id} is used twice"),
            RuleError::Invalid(why) => write!(f, "invalid rule: {why}"),
        }
    }
}

/// Parses and checks a rules file.
pub fn parse_rules(json: &str) -> Result<Vec<Rule>, RuleError> {
    let file: RuleFile = serde_json::from_str(json).map_err(|e| RuleError::Parse(e.to_string()))?;
    if file.version != 1 {
        return Err(RuleError::UnsupportedVersion(file.version));
    }
    let mut seen = std::collections::HashSet::new();
    for rule in &file.rules {
        if !seen.insert(rule.id.as_str()) {
            return Err(RuleError::DuplicateId(rule.id.clone()));
        }
        match rule.kind {
            Kind::Files if rule.paths.is_empty() => {
                return Err(RuleError::Invalid(format!("{} has no paths", rule.id)));
            }
            Kind::RecycleBin if !rule.permanent => {
                return Err(RuleError::Invalid(format!(
                    "{} empties the Recycle Bin, so it must be marked permanent",
                    rule.id
                )));
            }
            _ => {}
        }
        for path in &rule.paths {
            if !path.starts_with('{') {
                return Err(RuleError::Invalid(format!(
                    "{}: paths must start with a known folder variable",
                    rule.id
                )));
            }
            if path.split('\\').any(|seg| seg == ".." || seg == ".") {
                return Err(RuleError::Invalid(format!(
                    "{}: relative segments are not allowed",
                    rule.id
                )));
            }
        }
    }
    Ok(file.rules)
}

/// The built-in rules. They are checked by a unit test, so this cannot fail at runtime.
pub fn builtin_rules() -> Vec<Rule> {
    parse_rules(BUILTIN_RULES_JSON).expect("built-in cleanup rules are validated by tests")
}

/// Replaces `{NAME}` placeholders. Returns `None` if a variable is unknown or empty,
/// so a rule never falls back to a relative or root path.
pub fn expand_vars(template: &str, lookup: impl Fn(&str) -> Option<String>) -> Option<String> {
    let mut out = String::with_capacity(template.len() + 32);
    let mut rest = template;
    while let Some(start) = rest.find('{') {
        out.push_str(&rest[..start]);
        let end = rest[start..].find('}')? + start;
        let value = lookup(&rest[start + 1..end]).filter(|v| !v.trim().is_empty())?;
        out.push_str(value.trim_end_matches(['\\', '/']));
        rest = &rest[end + 1..];
    }
    out.push_str(rest);
    Some(out)
}

/// Case-insensitive match of a file name against a pattern with `*` and `?`.
pub fn glob_match(pattern: &str, name: &str) -> bool {
    let p: Vec<char> = pattern.to_lowercase().chars().collect();
    let n: Vec<char> = name.to_lowercase().chars().collect();
    let (mut pi, mut ni) = (0, 0);
    let (mut star, mut mark) = (None, 0);
    while ni < n.len() {
        if pi < p.len() && (p[pi] == '?' || p[pi] == n[ni]) {
            pi += 1;
            ni += 1;
        } else if pi < p.len() && p[pi] == '*' {
            star = Some(pi);
            mark = ni;
            pi += 1;
        } else if let Some(s) = star {
            pi = s + 1;
            mark += 1;
            ni = mark;
        } else {
            return false;
        }
    }
    while pi < p.len() && p[pi] == '*' {
        pi += 1;
    }
    pi == p.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_rules_are_valid() {
        let rules = parse_rules(BUILTIN_RULES_JSON).unwrap();
        assert!(rules.len() >= 10);
        assert!(
            rules
                .iter()
                .any(|r| r.kind == Kind::RecycleBin && r.permanent && !r.default_selected)
        );
        // Admin rules are never pre-selected: the elevated helper arrives in Phase 4.
        assert!(
            rules
                .iter()
                .filter(|r| r.admin)
                .all(|r| !r.default_selected)
        );
    }

    #[test]
    fn rejects_duplicates_and_relative_paths() {
        let dup = r#"{"version":1,"rules":[
            {"id":"a","group":"system","kind":"files","paths":["{TEMP}"],"safety":"safe"},
            {"id":"a","group":"system","kind":"files","paths":["{TEMP}"],"safety":"safe"}]}"#;
        assert_eq!(parse_rules(dup), Err(RuleError::DuplicateId("a".into())));

        let rel = r#"{"version":1,"rules":[
            {"id":"a","group":"system","kind":"files","paths":["{TEMP}\\..\\x"],"safety":"safe"}]}"#;
        assert!(matches!(parse_rules(rel), Err(RuleError::Invalid(_))));

        let bare = r#"{"version":1,"rules":[
            {"id":"a","group":"system","kind":"files","paths":["C:\\Windows"],"safety":"safe"}]}"#;
        assert!(matches!(parse_rules(bare), Err(RuleError::Invalid(_))));

        let unknown = r#"{"version":1,"rules":[
            {"id":"a","group":"system","kind":"files","paths":["{TEMP}"],"safety":"safe","oops":1}]}"#;
        assert!(matches!(parse_rules(unknown), Err(RuleError::Parse(_))));
    }

    #[test]
    fn expands_variables() {
        let env = |k: &str| match k {
            "TEMP" => Some(r"C:\Users\me\AppData\Local\Temp\".to_string()),
            "EMPTY" => Some("  ".to_string()),
            _ => None,
        };
        assert_eq!(
            expand_vars(r"{TEMP}\x", env).as_deref(),
            Some(r"C:\Users\me\AppData\Local\Temp\x")
        );
        assert_eq!(expand_vars(r"{NOPE}\x", env), None);
        assert_eq!(expand_vars(r"{EMPTY}\x", env), None);
        assert_eq!(expand_vars(r"{TEMP", env), None);
    }

    #[test]
    fn globs() {
        assert!(glob_match("thumbcache_*.db", "thumbcache_1280.db"));
        assert!(glob_match("thumbcache_*.db", "THUMBCACHE_idx.DB"));
        assert!(!glob_match("thumbcache_*.db", "thumbcache_1280.db-wal"));
        assert!(glob_match("a?c", "abc"));
        assert!(glob_match("*", "anything"));
        assert!(!glob_match("iconcache_*.db", "thumbcache_1.db"));
    }
}
