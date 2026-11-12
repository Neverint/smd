//! Toolbox catalog (`rules/toolbox.json`): trusted free tools installed through winget.

use serde::{Deserialize, Serialize};

use crate::tweaks::Text;

pub const BUILTIN_TOOLS_JSON: &str = include_str!("../../../rules/toolbox.json");

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenWith {
    /// Name in the Start menu's app list (`Get-StartApps`), matched exactly.
    #[serde(default)]
    pub start_app: Option<String>,
    /// File names (simple `*` globs) of the program, for portable tools that winget unpacks.
    #[serde(default)]
    pub exe: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Tool {
    pub id: String,
    pub winget_id: String,
    pub name: String,
    pub category: String,
    pub license: String,
    pub open_source: bool,
    pub homepage: String,
    pub description: Text,
    pub open: OpenWith,
}

#[derive(Debug, Deserialize)]
struct Catalog {
    version: u32,
    tools: Vec<Tool>,
}

fn valid(t: &Tool) -> bool {
    !t.id.is_empty()
        && t.id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        && crate::apps::is_safe_package_id(&t.winget_id)
        && t.homepage.starts_with("https://")
        && !t.description.en.is_empty()
        && !t.description.fr.is_empty()
        && t.open.exe.iter().all(|e| {
            e.to_ascii_lowercase().ends_with(".exe")
                && !e.contains(['\\', '/', ':'])
                && !e.contains("..")
        })
}

pub fn parse_tools(json: &str) -> Result<Vec<Tool>, String> {
    let c: Catalog = serde_json::from_str(json).map_err(|e| e.to_string())?;
    if c.version != 1 {
        return Err(format!("unsupported toolbox.json version {}", c.version));
    }
    let mut seen = std::collections::HashSet::new();
    for t in &c.tools {
        if !valid(t) {
            return Err(format!("tool {} is not valid", t.id));
        }
        if !seen.insert(t.id.clone()) {
            return Err(format!("duplicate tool {}", t.id));
        }
    }
    Ok(c.tools)
}

pub fn builtin_tools() -> Vec<Tool> {
    parse_tools(BUILTIN_TOOLS_JSON).expect("rules/toolbox.json is checked by tests")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_catalog_is_valid() {
        let tools = builtin_tools();
        assert!(tools.len() >= 12);
        assert!(
            tools
                .iter()
                .any(|t| t.winget_id == "Klocman.BulkCrapUninstaller")
        );
        assert!(tools.iter().all(|t| !t.license.is_empty()));
    }

    #[test]
    fn refuses_unsafe_ids_and_paths() {
        let base = r#"{"version":1,"tools":[{"id":"x","wingetId":"A.B","name":"X","category":"files","license":"MIT","openSource":true,"homepage":"https://x","description":{"en":"a","fr":"b"},"open":{"exe":["x.exe"]}}]}"#;
        assert!(parse_tools(base).is_ok());
        assert!(parse_tools(&base.replace("A.B", "A.B & calc")).is_err());
        assert!(parse_tools(&base.replace("x.exe", "..\\\\x.exe")).is_err());
        assert!(parse_tools(&base.replace("https://x", "http://x")).is_err());
    }
}
