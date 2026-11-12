//! User environment variables (`HKCU\Environment`), used to point a tool's cache to a new place.
//! Only the variables of the known caches can be changed.

use std::io;

fn allowed(name: &str) -> bool {
    smd_core::organizer::CACHES
        .iter()
        .any(|c| c.env_var.eq_ignore_ascii_case(name))
}

#[cfg(windows)]
mod imp {
    use super::*;
    use winreg::RegKey;
    use winreg::enums::{HKEY_CURRENT_USER, KEY_READ, KEY_SET_VALUE};

    pub fn get_user(name: &str) -> Option<String> {
        RegKey::predef(HKEY_CURRENT_USER)
            .open_subkey_with_flags("Environment", KEY_READ)
            .ok()?
            .get_value(name)
            .ok()
    }

    /// Sets (or removes, with `None`) a user variable and tells running programs about it.
    pub fn set_user(name: &str, value: Option<&str>) -> io::Result<()> {
        if !allowed(name) {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!("{name} is not a known cache variable"),
            ));
        }
        let key = RegKey::predef(HKEY_CURRENT_USER)
            .open_subkey_with_flags("Environment", KEY_READ | KEY_SET_VALUE)?;
        match value {
            Some(v) => key.set_value(name, &v)?,
            None => match key.delete_value(name) {
                Err(e) if e.kind() == io::ErrorKind::NotFound => {}
                r => r?,
            },
        }
        broadcast();
        Ok(())
    }

    fn broadcast() {
        use windows::Win32::Foundation::{LPARAM, WPARAM};
        use windows::Win32::UI::WindowsAndMessaging::{
            HWND_BROADCAST, SMTO_ABORTIFHUNG, SendMessageTimeoutW, WM_SETTINGCHANGE,
        };
        use windows::core::w;
        let env = w!("Environment");
        unsafe {
            let _ = SendMessageTimeoutW(
                HWND_BROADCAST,
                WM_SETTINGCHANGE,
                WPARAM(0),
                LPARAM(env.as_ptr() as isize),
                SMTO_ABORTIFHUNG,
                2000,
                None,
            );
        }
    }
}

#[cfg(not(windows))]
mod imp {
    use super::*;
    pub fn get_user(_name: &str) -> Option<String> {
        None
    }
    pub fn set_user(name: &str, _value: Option<&str>) -> io::Result<()> {
        if !allowed(name) {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "unknown variable",
            ));
        }
        Err(io::Error::new(io::ErrorKind::Unsupported, "Windows only"))
    }
}

pub use imp::*;

#[cfg(test)]
mod tests {
    #[test]
    fn only_known_cache_variables() {
        assert!(super::allowed("HF_HOME"));
        assert!(!super::allowed("PATH"));
        assert!(super::set_user("PATH", Some("x")).is_err());
        assert!(
            super::set_user("TEMP", None).is_err(),
            "TEMP is never changed"
        );
    }
}
