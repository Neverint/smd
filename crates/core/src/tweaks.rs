//! Privacy & Tweaks catalog (`rules/tweaks.json`): parsing, validation and state detection.
//!
//! Every tweak is declarative: registry values, service start types or one Store app to remove.
//! Nothing here runs a command, so the elevated helper can only apply what this catalog lists.

use serde::{Deserialize, Serialize};

pub const BUILTIN_TWEAKS_JSON: &str = include_str!("../../../rules/tweaks.json");

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Category {
    Privacy,
    Ads,
    Ai,
    Explorer,
    Apps,
}

/// How careful to be. `caution` tweaks are never pre-selected and get a warning.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Risk {
    Safe,
    Advanced,
    Caution,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Text {
    pub en: String,
    pub fr: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Hive {
    Hkcu,
    Hklm,
}

/// A registry value: a JSON number is a `REG_DWORD`, a JSON string a `REG_SZ`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RegData {
    Dword(u32),
    Sz(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegSetting {
    pub hive: Hive,
    /// Key under the hive, without a leading backslash.
    pub key: String,
    /// Value name; empty for the key's default value.
    #[serde(default)]
    pub name: String,
    pub value: RegData,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StartType {
    Automatic,
    Manual,
    Disabled,
}

impl StartType {
    /// The `Start` value under `HKLM\SYSTEM\CurrentControlSet\Services\<name>`.
    pub fn from_registry(v: u32) -> Option<StartType> {
        match v {
            2 => Some(StartType::Automatic),
            3 => Some(StartType::Manual),
            4 => Some(StartType::Disabled),
            _ => None,
        }
    }

    pub fn registry_value(self) -> u32 {
        match self {
            StartType::Automatic => 2,
            StartType::Manual => 3,
            StartType::Disabled => 4,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceSetting {
    pub name: String,
    pub start: StartType,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Tweak {
    pub id: String,
    pub category: Category,
    pub risk: Risk,
    pub title: Text,
    pub description: Text,
    /// File Explorer must restart (or you sign out) before the change shows.
    #[serde(default)]
    pub restart_explorer: bool,
    /// Takes effect after a restart of Windows.
    #[serde(default)]
    pub restart_windows: bool,
    #[serde(default)]
    pub registry: Vec<RegSetting>,
    #[serde(default)]
    pub services: Vec<ServiceSetting>,
    /// A Store app (package `Name`) removed for your account.
    #[serde(default)]
    pub appx: Option<String>,
}

impl Tweak {
    /// Machine-wide values and services need the elevated helper.
    pub fn needs_admin(&self) -> bool {
        !self.services.is_empty() || self.registry.iter().any(|r| r.hive == Hive::Hklm)
    }

    pub fn user_registry(&self) -> impl Iterator<Item = &RegSetting> {
        self.registry.iter().filter(|r| r.hive == Hive::Hkcu)
    }

    pub fn machine_registry(&self) -> impl Iterator<Item = &RegSetting> {
        self.registry.iter().filter(|r| r.hive == Hive::Hklm)
    }
}

#[derive(Debug, Deserialize)]
struct Catalog {
    version: u32,
    tweaks: Vec<Tweak>,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum TweakError {
    #[error("tweaks.json is not valid: {0}")]
    Json(String),
    #[error("unsupported tweaks.json version {0}")]
    Version(u32),
    #[error("tweak {0}: {1}")]
    Invalid(String, &'static str),
}

/// Registry keys a tweak may write. Anything else is refused when the catalog loads.
const ALLOWED_USER_PREFIXES: &[&str] = &["Software\\"];
const ALLOWED_MACHINE_PREFIXES: &[&str] = &["SOFTWARE\\Policies\\"];

fn has_prefix(key: &str, prefixes: &[&str]) -> bool {
    prefixes
        .iter()
        .any(|p| key.len() > p.len() && key[..p.len()].eq_ignore_ascii_case(p))
}

fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

fn valid_service_name(name: &str) -> bool {
    !name.is_empty() && name.len() <= 64 && name.chars().all(|c| c.is_ascii_alphanumeric())
}

fn valid_key(key: &str) -> bool {
    !key.is_empty()
        && !key.starts_with('\\')
        && !key.ends_with('\\')
        && !key.contains("\\\\")
        && !key.contains("..")
        && key.chars().all(|c| !c.is_control())
}

fn validate(t: &Tweak) -> Result<(), TweakError> {
    let bad = |why| Err(TweakError::Invalid(t.id.clone(), why));
    if !valid_id(&t.id) {
        return bad("id must be lower-case letters, digits and dashes");
    }
    for text in [&t.title, &t.description] {
        if text.en.trim().is_empty() || text.fr.trim().is_empty() {
            return bad("title and description need English and French");
        }
    }
    let kinds = usize::from(!t.registry.is_empty())
        + usize::from(!t.services.is_empty())
        + usize::from(t.appx.is_some());
    if kinds == 0 {
        return bad("a tweak must change something");
    }
    if t.appx.is_some() && kinds > 1 {
        return bad("an app removal cannot be mixed with other changes");
    }
    for r in &t.registry {
        if !valid_key(&r.key) || r.name.chars().any(char::is_control) {
            return bad("bad registry key or value name");
        }
        let ok = match r.hive {
            Hive::Hkcu => has_prefix(&r.key, ALLOWED_USER_PREFIXES),
            Hive::Hklm => has_prefix(&r.key, ALLOWED_MACHINE_PREFIXES),
        };
        if !ok {
            return bad("registry key outside the allowed places");
        }
    }
    for s in &t.services {
        if !valid_service_name(&s.name) {
            return bad("bad service name");
        }
    }
    if let Some(a) = &t.appx
        && (!crate::apps::is_safe_appx_name(a) || a.contains('_'))
    {
        return bad("bad app package name");
    }
    Ok(())
}

pub fn parse_tweaks(json: &str) -> Result<Vec<Tweak>, TweakError> {
    let catalog: Catalog =
        serde_json::from_str(json).map_err(|e| TweakError::Json(e.to_string()))?;
    if catalog.version != 1 {
        return Err(TweakError::Version(catalog.version));
    }
    let mut seen = std::collections::HashSet::new();
    for t in &catalog.tweaks {
        validate(t)?;
        if !seen.insert(t.id.clone()) {
            return Err(TweakError::Invalid(t.id.clone(), "duplicate id"));
        }
    }
    Ok(catalog.tweaks)
}

/// The catalog shipped with the app (checked by a unit test).
pub fn builtin_tweaks() -> Vec<Tweak> {
    parse_tweaks(BUILTIN_TWEAKS_JSON).expect("rules/tweaks.json is checked by tests")
}

pub fn find(id: &str) -> Option<Tweak> {
    builtin_tweaks().into_iter().find(|t| t.id == id)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TweakState {
    /// Every value is already as the tweak sets it.
    Applied,
    NotApplied,
    /// Some values are set, others are not.
    Partial,
    /// The app this tweak removes is not installed.
    NotInstalled,
    /// Could not be read.
    Unknown,
}

/// State from the current registry values and service start types (in catalog order).
/// `None` means the value is absent or has another type.
pub fn settings_state(
    tweak: &Tweak,
    registry: &[Option<RegData>],
    services: &[Option<StartType>],
) -> TweakState {
    let mut matches = 0usize;
    let mut total = 0usize;
    for (want, have) in tweak.registry.iter().zip(registry) {
        total += 1;
        if have.as_ref() == Some(&want.value) {
            matches += 1;
        }
    }
    for (want, have) in tweak.services.iter().zip(services) {
        total += 1;
        if *have == Some(want.start) {
            matches += 1;
        }
    }
    if total == 0 {
        TweakState::Unknown
    } else if matches == total {
        TweakState::Applied
    } else if matches == 0 {
        TweakState::NotApplied
    } else {
        TweakState::Partial
    }
}

/// A registry value as it was before a tweak, for the undo. `vtype` and `bytes` are raw.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawValue {
    pub vtype: u32,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegPrevious {
    pub hive: Hive,
    pub key: String,
    pub name: String,
    /// `None`: the value did not exist (the undo deletes it).
    pub previous: Option<RawValue>,
    /// The highest key the tweak created (deleted on undo if it is empty again).
    pub created_key: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServicePrevious {
    pub name: String,
    pub start: u32,
    /// Automatic (delayed start).
    #[serde(default)]
    pub delayed: bool,
}

/// What an undo of the machine-wide part needs (made and replayed by the elevated helper).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MachineUndo {
    pub registry: Vec<RegPrevious>,
    pub services: Vec<ServicePrevious>,
}

/// The helper only replays an undo that touches values and services this tweak declares.
pub fn undo_matches(tweak: &Tweak, undo: &MachineUndo) -> bool {
    let reg_ok = undo.registry.iter().all(|p| {
        p.hive == Hive::Hklm
            && tweak
                .machine_registry()
                .any(|r| r.key.eq_ignore_ascii_case(&p.key) && r.name.eq_ignore_ascii_case(&p.name))
            && p.created_key.as_deref().is_none_or(|c| {
                // The created key must be the tweak's key or one of its parents.
                tweak.machine_registry().any(|r| {
                    r.key.eq_ignore_ascii_case(c)
                        || (r.key.len() > c.len()
                            && r.key[..c.len()].eq_ignore_ascii_case(c)
                            && r.key.as_bytes()[c.len()] == b'\\')
                })
            })
    });
    let svc_ok = undo.services.iter().all(|p| {
        StartType::from_registry(p.start).is_some()
            && tweak
                .services
                .iter()
                .any(|s| s.name.eq_ignore_ascii_case(&p.name))
    });
    reg_ok && svc_ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_catalog_is_valid() {
        let all = builtin_tweaks();
        assert!(all.len() >= 20);
        assert!(all.iter().any(|t| t.needs_admin()));
        assert!(all.iter().any(|t| t.appx.is_some()));
        assert!(
            all.iter()
                .filter(|t| t.appx.is_some())
                .all(|t| !t.needs_admin()),
            "app removals run for your account, without admin"
        );
    }

    fn one(json_tweak: &str) -> Result<Vec<Tweak>, TweakError> {
        parse_tweaks(&format!(r#"{{"version":1,"tweaks":[{json_tweak}]}}"#))
    }

    const TEXT: &str = r#""title":{"en":"T","fr":"T"},"description":{"en":"D","fr":"D"}"#;

    #[test]
    fn refuses_registry_outside_the_allowed_places() {
        let ok = one(&format!(
            r#"{{"id":"a","category":"privacy","risk":"safe",{TEXT},"registry":[{{"hive":"hklm","key":"SOFTWARE\\Policies\\X","name":"Y","value":0}}]}}"#
        ));
        assert!(ok.is_ok(), "{ok:?}");
        for key in [
            r"SYSTEM\\CurrentControlSet\\Services\\X",
            r"SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\\Winlogon",
            r"SOFTWARE\\Policies\\..\\X",
        ] {
            let bad = one(&format!(
                r#"{{"id":"a","category":"privacy","risk":"safe",{TEXT},"registry":[{{"hive":"hklm","key":"{key}","name":"Y","value":0}}]}}"#
            ));
            assert!(bad.is_err(), "{key} must be refused");
        }
    }

    #[test]
    fn refuses_mixed_app_removal_and_bad_names() {
        let mixed = one(&format!(
            r#"{{"id":"a","category":"apps","risk":"safe",{TEXT},"appx":"Microsoft.BingNews","registry":[{{"hive":"hkcu","key":"Software\\X","name":"Y","value":0}}]}}"#
        ));
        assert!(mixed.is_err());
        let bad = one(&format!(
            r#"{{"id":"a","category":"apps","risk":"safe",{TEXT},"appx":"Microsoft.Bing;News"}}"#
        ));
        assert!(bad.is_err());
        let svc = one(&format!(
            r#"{{"id":"a","category":"privacy","risk":"safe",{TEXT},"services":[{{"name":"Diag Track","start":"disabled"}}]}}"#
        ));
        assert!(svc.is_err());
    }

    #[test]
    fn untagged_values_and_state() {
        let t = &one(&format!(
            r#"{{"id":"a","category":"explorer","risk":"advanced",{TEXT},"registry":[{{"hive":"hkcu","key":"Software\\X","name":"A","value":0}},{{"hive":"hkcu","key":"Software\\X","value":""}}]}}"#
        ))
        .unwrap()[0];
        assert_eq!(t.registry[0].value, RegData::Dword(0));
        assert_eq!(t.registry[1].value, RegData::Sz(String::new()));
        assert!(!t.needs_admin());
        let applied = [Some(RegData::Dword(0)), Some(RegData::Sz(String::new()))];
        assert_eq!(settings_state(t, &applied, &[]), TweakState::Applied);
        let half = [Some(RegData::Dword(0)), None];
        assert_eq!(settings_state(t, &half, &[]), TweakState::Partial);
        assert_eq!(
            settings_state(t, &[None, None], &[]),
            TweakState::NotApplied
        );
    }

    #[test]
    fn undo_must_match_the_tweak() {
        let t = &one(&format!(
            r#"{{"id":"a","category":"privacy","risk":"safe",{TEXT},"registry":[{{"hive":"hklm","key":"SOFTWARE\\Policies\\A\\B","name":"V","value":1}}],"services":[{{"name":"DiagTrack","start":"disabled"}}]}}"#
        ))
        .unwrap()[0];
        let good = MachineUndo {
            registry: vec![RegPrevious {
                hive: Hive::Hklm,
                key: r"SOFTWARE\Policies\A\B".into(),
                name: "V".into(),
                previous: None,
                created_key: Some(r"SOFTWARE\Policies\A".into()),
            }],
            services: vec![ServicePrevious {
                name: "DiagTrack".into(),
                start: 2,
                delayed: false,
            }],
        };
        assert!(undo_matches(t, &good));
        let mut other_value = good.clone();
        other_value.registry[0].name = "Other".into();
        assert!(!undo_matches(t, &other_value));
        let mut wide_delete = good.clone();
        wide_delete.registry[0].created_key = Some(r"SOFTWARE\Pol".into());
        assert!(!undo_matches(t, &wide_delete));
        let mut other_service = good;
        other_service.services[0].name = "WinDefend".into();
        assert!(!undo_matches(t, &other_service));
    }
}
