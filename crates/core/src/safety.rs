//! Last-line checks before anything is deleted or moved, whatever a rule says.
//!
//! Paths are compared as Windows paths: case-insensitive, `/` and `\` equal,
//! trailing separators ignored.

/// Folders that must never be deleted, nor any folder that contains them.
/// `{SYSTEMROOT}` style roots are matched on any drive letter.
const NEVER_DELETE: &[&str] = &[
    r"\windows",
    r"\windows\system32",
    r"\windows\syswow64",
    r"\windows\winsxs",
    r"\program files",
    r"\program files (x86)",
    r"\programdata",
    r"\users",
    r"\recovery",
    r"\boot",
    r"\efi",
];

/// Folders whose content must never be deleted either.
const NEVER_INSIDE: &[&str] = &[
    r"\windows\system32",
    r"\windows\syswow64",
    r"\windows\winsxs",
    r"\boot",
    r"\efi",
    r"\recovery",
];

/// Lower-case, backslashes, no trailing separator, `\\?\` prefix removed.
pub fn normalize(path: &str) -> String {
    let mut p = path.replace('/', "\\").to_lowercase();
    if let Some(rest) = p.strip_prefix(r"\\?\") {
        p = rest.to_string();
    }
    while p.ends_with('\\') && p.len() > 3 {
        p.pop();
    }
    p
}

/// True if `child` is strictly inside `parent`.
pub fn is_inside(child: &str, parent: &str) -> bool {
    let c = normalize(child);
    let p = normalize(parent);
    c.len() > p.len() + 1 && c.starts_with(&p) && c.as_bytes()[p.len()] == b'\\'
}

pub fn same_path(a: &str, b: &str) -> bool {
    normalize(a) == normalize(b)
}

/// Strips a drive letter (`c:`), returning the rest (`\windows`). `None` for non-drive paths.
fn drive_relative(normalized: &str) -> Option<&str> {
    let bytes = normalized.as_bytes();
    if bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' {
        Some(&normalized[2..])
    } else {
        None
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum Refusal {
    NotAbsolute,
    RelativeSegment,
    TooShallow,
    Protected,
    OutsideRoot,
}

/// Checks that `item` may be removed as part of a rule whose root is `root`.
///
/// `item_is_root` is true for rules that remove the root folder itself (browser caches).
pub fn check_removal(item: &str, root: &str, item_is_root: bool) -> Result<(), Refusal> {
    let n = normalize(item);
    let Some(rel) = drive_relative(&n) else {
        return Err(Refusal::NotAbsolute);
    };
    if !rel.starts_with('\\') {
        return Err(Refusal::NotAbsolute);
    }
    if rel.split('\\').any(|seg| seg == ".." || seg == ".") {
        return Err(Refusal::RelativeSegment);
    }
    // At least drive\a\b: never a drive root or a top-level folder.
    if rel.split('\\').filter(|s| !s.is_empty()).count() < 2 {
        return Err(Refusal::TooShallow);
    }
    for never in NEVER_DELETE {
        if rel == *never || never.starts_with(&format!("{rel}\\")) {
            return Err(Refusal::Protected);
        }
    }
    for never in NEVER_INSIDE {
        if rel.starts_with(&format!("{never}\\")) {
            return Err(Refusal::Protected);
        }
    }
    let inside = if item_is_root {
        same_path(item, root)
    } else {
        is_inside(item, root)
    };
    if inside {
        Ok(())
    } else {
        Err(Refusal::OutsideRoot)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEMP: &str = r"C:\Users\me\AppData\Local\Temp";

    #[test]
    fn allows_items_inside_the_root() {
        assert_eq!(
            check_removal(r"C:\Users\me\AppData\Local\Temp\abc", TEMP, false),
            Ok(())
        );
        assert_eq!(
            check_removal(r"c:/users/ME/appdata/local/temp/abc/", TEMP, false),
            Ok(())
        );
        assert_eq!(
            check_removal(r"C:\Windows\Temp\x.log", r"C:\Windows\Temp", false),
            Ok(())
        );
        assert_eq!(
            check_removal(r"C:\Windows\MEMORY.DMP", r"C:\Windows\MEMORY.DMP", true),
            Ok(())
        );
    }

    #[test]
    fn refuses_the_root_itself_for_children_rules() {
        assert_eq!(check_removal(TEMP, TEMP, false), Err(Refusal::OutsideRoot));
        assert_eq!(
            check_removal(r"C:\Users\me\AppData\Local\Tempo\x", TEMP, false),
            Err(Refusal::OutsideRoot)
        );
    }

    #[test]
    fn refuses_protected_and_shallow_paths() {
        assert_eq!(
            check_removal(r"C:\", r"C:\", true),
            Err(Refusal::TooShallow)
        );
        assert_eq!(
            check_removal(r"C:\Windows", r"C:\", false),
            Err(Refusal::TooShallow)
        );
        assert_eq!(
            check_removal(r"C:\Windows\WinSxS", r"C:\Windows", false),
            Err(Refusal::Protected)
        );
        assert_eq!(
            check_removal(
                r"C:\Windows\System32\drivers",
                r"C:\Windows\System32",
                false
            ),
            Err(Refusal::Protected)
        );
        assert_eq!(
            check_removal(r"D:\Program Files\App", r"D:\Program Files", false),
            Ok(())
        );
        assert_eq!(
            check_removal(r"C:\Program Files (x86)", r"C:\", false),
            Err(Refusal::TooShallow)
        );
    }

    #[test]
    fn refuses_relative_and_unc_paths() {
        assert_eq!(
            check_removal(r"C:\Users\me\AppData\Local\Temp\..\x", TEMP, false),
            Err(Refusal::RelativeSegment)
        );
        assert_eq!(
            check_removal(r"\\server\share\x\y", r"\\server\share", false),
            Err(Refusal::NotAbsolute)
        );
        assert_eq!(
            check_removal(r"Temp\x", "Temp", false),
            Err(Refusal::NotAbsolute)
        );
    }

    #[test]
    fn inside_checks_separators() {
        assert!(is_inside(r"C:\a\b", r"C:\a"));
        assert!(!is_inside(r"C:\ab", r"C:\a"));
        assert!(!is_inside(r"C:\a", r"C:\a"));
        assert!(is_inside(r"\\?\C:\a\b", r"C:\a\"));
    }
}
