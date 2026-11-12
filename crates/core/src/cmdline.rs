//! Reading program paths out of Windows command lines (Run keys, services, uninstall strings).
//!
//! These strings come from the registry, so they are treated as data: we only ever split
//! them into a program path and its arguments, never hand them to a shell.

/// Expands `%NAME%` variables. Unknown variables are left as they are.
pub fn expand_env(s: &str, lookup: impl Fn(&str) -> Option<String>) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(start) = rest.find('%') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        match after.find('%') {
            Some(end) => {
                let name = &after[..end];
                match lookup(name).filter(|_| !name.is_empty()) {
                    Some(v) => out.push_str(&v),
                    None => {
                        out.push('%');
                        out.push_str(name);
                        out.push('%');
                    }
                }
                rest = &after[end + 1..];
            }
            None => {
                out.push_str(&rest[start..]);
                rest = "";
            }
        }
    }
    out.push_str(rest);
    out
}

/// Splits a command line into (program, arguments).
///
/// - `"C:\Path With Spaces\app.exe" /arg` → quoted program.
/// - `C:\Program Files\App\app.exe /S` (unquoted, with spaces) → the shortest prefix
///   ending in `.exe` for which `exists` is true; otherwise the first `.exe` prefix.
/// - `rundll32.exe shell32.dll,Control_RunDLL` → first token.
pub fn split_command(cmd: &str, exists: impl Fn(&str) -> bool) -> Option<(String, String)> {
    let cmd = cmd.trim();
    if cmd.is_empty() {
        return None;
    }
    if let Some(rest) = cmd.strip_prefix('"') {
        let end = rest.find('"')?;
        let program = rest[..end].trim().to_string();
        let args = rest[end + 1..].trim().to_string();
        return (!program.is_empty()).then_some((program, args));
    }
    let lower = cmd.to_ascii_lowercase();
    let mut first_exe = None;
    let mut search = 0;
    while let Some(i) = lower[search..].find(".exe") {
        let end = search + i + 4;
        let at_boundary = end == cmd.len() || cmd[end..].starts_with(' ');
        if at_boundary {
            let program = &cmd[..end];
            if exists(program) {
                return Some((program.to_string(), cmd[end..].trim().to_string()));
            }
            first_exe.get_or_insert(end);
        }
        search = end;
    }
    if let Some(end) = first_exe {
        return Some((cmd[..end].to_string(), cmd[end..].trim().to_string()));
    }
    match cmd.split_once(' ') {
        Some((p, a)) => Some((p.to_string(), a.trim().to_string())),
        None => Some((cmd.to_string(), String::new())),
    }
}

/// The MSI product code (`{GUID}`) of an `MsiExec.exe /I{...}` or `/X{...}` uninstall string.
pub fn msi_product_code(cmd: &str) -> Option<String> {
    let lower = cmd.to_ascii_lowercase();
    if !lower.contains("msiexec") {
        return None;
    }
    let start = cmd.find('{')?;
    let end = cmd[start..].find('}')? + start;
    let guid = &cmd[start..=end];
    is_guid(guid).then(|| guid.to_uppercase())
}

/// `{8-4-4-4-12}` hex GUID with braces.
pub fn is_guid(s: &str) -> bool {
    let b = s.as_bytes();
    if b.len() != 38 || b[0] != b'{' || b[37] != b'}' {
        return false;
    }
    s[1..37].char_indices().all(|(i, c)| match i {
        8 | 13 | 18 | 23 => c == '-',
        _ => c.is_ascii_hexdigit(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expands_variables() {
        let env = |k: &str| {
            (k.eq_ignore_ascii_case("ProgramFiles")).then(|| r"C:\Program Files".to_string())
        };
        assert_eq!(
            expand_env(r"%ProgramFiles%\App\a.exe", env),
            r"C:\Program Files\App\a.exe"
        );
        assert_eq!(expand_env(r"%NOPE%\a.exe", env), r"%NOPE%\a.exe");
        assert_eq!(expand_env("50% off", env), "50% off");
    }

    #[test]
    fn splits_quoted_and_unquoted_commands() {
        let none = |_: &str| false;
        assert_eq!(
            split_command(r#""C:\Program Files\App\app.exe" --minimized"#, none),
            Some((r"C:\Program Files\App\app.exe".into(), "--minimized".into()))
        );
        let exists = |p: &str| p == r"C:\Program Files\App\uninst.exe";
        assert_eq!(
            split_command(r"C:\Program Files\App\uninst.exe /S /lang=en.exe", exists),
            Some((
                r"C:\Program Files\App\uninst.exe".into(),
                "/S /lang=en.exe".into()
            ))
        );
        assert_eq!(
            split_command("rundll32.exe shell32.dll,Control_RunDLL", none),
            Some(("rundll32.exe".into(), "shell32.dll,Control_RunDLL".into()))
        );
        assert_eq!(
            split_command("app", none),
            Some(("app".into(), String::new()))
        );
        assert_eq!(split_command("   ", none), None);
        assert_eq!(split_command("\"unterminated", none), None);
    }

    #[test]
    fn finds_msi_product_codes() {
        assert_eq!(
            msi_product_code("MsiExec.exe /I{3c4a2c9d-1b2e-4f5a-8a9b-0c1d2e3f4a5b}"),
            Some("{3C4A2C9D-1B2E-4F5A-8A9B-0C1D2E3F4A5B}".into())
        );
        assert_eq!(msi_product_code("MsiExec.exe /X{not-a-guid}"), None);
        assert_eq!(
            msi_product_code(r"C:\x\{3c4a2c9d-1b2e-4f5a-8a9b-0c1d2e3f4a5b}\u.exe"),
            None
        );
        assert!(!is_guid("{3c4a2c9d-1b2e-4f5a-8a9b-0c1d2e3f4a5b"));
    }
}
