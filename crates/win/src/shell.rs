//! Small Win32 helpers: a file's publisher, known folders, launching a program.

use std::io;
use std::path::PathBuf;

/// Known folders used by My Clutter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KnownFolder {
    Desktop,
    Documents,
    Downloads,
    Pictures,
    Music,
    Videos,
}

#[cfg(windows)]
mod imp {
    use super::*;
    use windows::Win32::Foundation::{CloseHandle, WAIT_OBJECT_0};
    use windows::Win32::Storage::FileSystem::{
        GetFileVersionInfoSizeW, GetFileVersionInfoW, VerQueryValueW,
    };
    use windows::Win32::System::Com::CoTaskMemFree;
    use windows::Win32::System::Threading::{GetExitCodeProcess, INFINITE, WaitForSingleObject};
    use windows::Win32::UI::Shell::{
        FOLDERID_Desktop, FOLDERID_Documents, FOLDERID_Downloads, FOLDERID_Music,
        FOLDERID_Pictures, FOLDERID_Videos, KF_FLAG_DEFAULT, SEE_MASK_NOCLOSEPROCESS,
        SHELLEXECUTEINFOW, SHGetKnownFolderPath, ShellExecuteExW,
    };
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
    use windows::core::{HSTRING, PCWSTR};

    /// The `CompanyName` in a file's version resource ("Mozilla Corporation").
    pub fn file_company(path: &str) -> Option<String> {
        let p = HSTRING::from(path);
        let size = unsafe { GetFileVersionInfoSizeW(&p, None) };
        if size == 0 {
            return None;
        }
        let mut buf = vec![0u8; size as usize];
        unsafe { GetFileVersionInfoW(&p, None, size, buf.as_mut_ptr().cast()) }.ok()?;

        let mut ptr = std::ptr::null_mut();
        let mut len = 0u32;
        let mut langs = Vec::new();
        if unsafe {
            VerQueryValueW(
                buf.as_ptr().cast(),
                &HSTRING::from(r"\VarFileInfo\Translation"),
                &mut ptr,
                &mut len,
            )
        }
        .as_bool()
            && len >= 4
        {
            let pairs =
                unsafe { std::slice::from_raw_parts(ptr as *const u16, (len / 2) as usize) };
            for pair in pairs.as_chunks::<2>().0 {
                langs.push(format!("{:04x}{:04x}", pair[0], pair[1]));
            }
        }
        langs.extend([
            "040904b0".to_string(),
            "040904e4".to_string(),
            "04090000".to_string(),
        ]);
        for lang in langs {
            let key = HSTRING::from(format!(r"\StringFileInfo\{lang}\CompanyName"));
            let mut ptr = std::ptr::null_mut();
            let mut len = 0u32;
            if unsafe { VerQueryValueW(buf.as_ptr().cast(), &key, &mut ptr, &mut len) }.as_bool()
                && len > 1
            {
                let s = unsafe { std::slice::from_raw_parts(ptr as *const u16, len as usize) };
                let end = s.iter().position(|&c| c == 0).unwrap_or(s.len());
                let v = String::from_utf16_lossy(&s[..end]).trim().to_string();
                if !v.is_empty() {
                    return Some(v);
                }
            }
        }
        None
    }

    pub fn known_folder(f: KnownFolder) -> Option<PathBuf> {
        let id = match f {
            KnownFolder::Desktop => &FOLDERID_Desktop,
            KnownFolder::Documents => &FOLDERID_Documents,
            KnownFolder::Downloads => &FOLDERID_Downloads,
            KnownFolder::Pictures => &FOLDERID_Pictures,
            KnownFolder::Music => &FOLDERID_Music,
            KnownFolder::Videos => &FOLDERID_Videos,
        };
        let pw = unsafe { SHGetKnownFolderPath(id, KF_FLAG_DEFAULT, None) }.ok()?;
        let s = unsafe { pw.to_string() }.ok();
        unsafe { CoTaskMemFree(Some(pw.0 as *const _)) };
        s.map(PathBuf::from)
    }

    /// Starts `program` with `args` through the shell (so the program's own manifest can ask
    /// for admin rights with UAC), waits for it and returns its exit code. No command shell is involved.
    pub fn run_and_wait(program: &str, args: &str) -> io::Result<u32> {
        let file = HSTRING::from(program);
        let params = HSTRING::from(args);
        let verb = HSTRING::from("open");
        let mut info = SHELLEXECUTEINFOW {
            cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
            fMask: SEE_MASK_NOCLOSEPROCESS,
            lpVerb: PCWSTR(verb.as_ptr()),
            lpFile: PCWSTR(file.as_ptr()),
            lpParameters: if args.is_empty() {
                PCWSTR::null()
            } else {
                PCWSTR(params.as_ptr())
            },
            nShow: SW_SHOWNORMAL.0,
            ..Default::default()
        };
        unsafe { ShellExecuteExW(&mut info) }.map_err(io::Error::other)?;
        let h = info.hProcess;
        if h.is_invalid() {
            return Ok(0);
        }
        let wait = unsafe { WaitForSingleObject(h, INFINITE) };
        let mut code = 0u32;
        if wait == WAIT_OBJECT_0 {
            let _ = unsafe { GetExitCodeProcess(h, &mut code) };
        }
        let _ = unsafe { CloseHandle(h) };
        Ok(code)
    }
}

#[cfg(not(windows))]
mod imp {
    use super::*;
    pub fn file_company(_path: &str) -> Option<String> {
        None
    }
    pub fn known_folder(_f: KnownFolder) -> Option<PathBuf> {
        None
    }
    pub fn run_and_wait(_program: &str, _args: &str) -> io::Result<u32> {
        Err(io::Error::new(io::ErrorKind::Unsupported, "Windows only"))
    }
}

pub use imp::*;

/// Environment lookup for `%VAR%` expansion (case-insensitive on Windows).
pub fn env_lookup(name: &str) -> Option<String> {
    std::env::var(name).ok()
}

/// The program of a command line, with variables expanded, if it exists on disk.
pub fn program_of(command: &str) -> Option<String> {
    let expanded = smd_core::cmdline::expand_env(command, env_lookup);
    let (program, _) =
        smd_core::cmdline::split_command(&expanded, |p| std::path::Path::new(p).is_file())?;
    let p = std::path::Path::new(&program);
    if p.is_absolute() {
        return p.is_file().then_some(program);
    }
    // Bare names (rundll32.exe): look in System32.
    let sys = std::env::var("WINDIR")
        .ok()
        .map(|w| PathBuf::from(w).join("System32").join(&program))?;
    sys.is_file().then(|| sys.to_string_lossy().into_owned())
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn reads_publishers_and_folders() {
        let notepad = format!(r"{}\System32\notepad.exe", std::env::var("WINDIR").unwrap());
        assert!(file_company(&notepad).unwrap().contains("Microsoft"));
        assert!(file_company(r"C:\does\not\exist.exe").is_none());
        assert!(known_folder(KnownFolder::Downloads).is_some());
        assert_eq!(
            program_of(r#""%WINDIR%\System32\notepad.exe" /x"#).as_deref(),
            Some(notepad.as_str())
        );
        assert!(
            program_of("rundll32.exe shell32.dll,Foo")
                .unwrap()
                .to_lowercase()
                .ends_with(r"system32\rundll32.exe")
        );
    }
}
