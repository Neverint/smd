//! Which programs have files open, through the Restart Manager (the API installers use).

use std::io;
use std::path::Path;

/// Names of the programs that hold any of `files` open. At most 1000 files are checked.
#[cfg(windows)]
pub fn locked_by(files: &[&Path]) -> io::Result<Vec<String>> {
    use windows::Win32::Foundation::{ERROR_MORE_DATA, ERROR_SUCCESS, WIN32_ERROR};
    use windows::Win32::System::RestartManager::{
        CCH_RM_SESSION_KEY, RM_PROCESS_INFO, RmEndSession, RmGetList, RmRegisterResources,
        RmStartSession,
    };
    use windows::core::{HSTRING, PCWSTR, PWSTR};

    if files.is_empty() {
        return Ok(Vec::new());
    }
    let mut session = 0u32;
    let mut key = [0u16; CCH_RM_SESSION_KEY as usize + 1];
    let err = unsafe { RmStartSession(&mut session, None, PWSTR(key.as_mut_ptr())) };
    if err != ERROR_SUCCESS {
        return Err(io::Error::other(format!("Restart Manager: {}", err.0)));
    }
    let result = (|| {
        let names: Vec<HSTRING> = files
            .iter()
            .take(1000)
            .map(|p| HSTRING::from(p.as_os_str()))
            .collect();
        let ptrs: Vec<PCWSTR> = names.iter().map(|h| PCWSTR(h.as_ptr())).collect();
        let err = unsafe { RmRegisterResources(session, Some(&ptrs), None, None) };
        if err != ERROR_SUCCESS {
            return Err(io::Error::other(format!("Restart Manager: {}", err.0)));
        }
        let mut needed = 0u32;
        let mut count = 0u32;
        let mut reasons = 0u32;
        let mut infos: Vec<RM_PROCESS_INFO> = Vec::new();
        loop {
            let err: WIN32_ERROR = unsafe {
                RmGetList(
                    session,
                    &mut needed,
                    &mut count,
                    Some(infos.as_mut_ptr()),
                    &mut reasons,
                )
            };
            if err == ERROR_MORE_DATA {
                infos = vec![RM_PROCESS_INFO::default(); needed as usize];
                count = needed;
                continue;
            }
            if err != ERROR_SUCCESS {
                return Err(io::Error::other(format!("Restart Manager: {}", err.0)));
            }
            break;
        }
        let mut names: Vec<String> = infos
            .iter()
            .take(count as usize)
            .map(|i| {
                let end = i
                    .strAppName
                    .iter()
                    .position(|&c| c == 0)
                    .unwrap_or(i.strAppName.len());
                String::from_utf16_lossy(&i.strAppName[..end])
            })
            .filter(|n| !n.is_empty())
            .collect();
        names.sort();
        names.dedup();
        Ok(names)
    })();
    unsafe {
        let _ = RmEndSession(session);
    }
    result
}

#[cfg(not(windows))]
pub fn locked_by(_files: &[&Path]) -> io::Result<Vec<String>> {
    Ok(Vec::new())
}

#[cfg(all(test, windows))]
mod tests {
    #[test]
    fn reports_the_program_holding_a_file() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("held.bin");
        std::fs::write(&p, b"x").unwrap();
        assert!(super::locked_by(&[p.as_path()]).unwrap().is_empty());
        // This test process keeps it open: Restart Manager names us.
        let _held = std::fs::OpenOptions::new().read(true).open(&p).unwrap();
        let names = super::locked_by(&[p.as_path()]).unwrap();
        assert!(!names.is_empty(), "the test runner should be listed");
    }
}
