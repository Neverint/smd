//! Folder watcher for the ongoing guard: `ReadDirectoryChangesW` on a few top-level folders
//! (game libraries, your profile). Only folder creations and renames are reported; the
//! app then waits a while and runs the Drive Organizer audit.

use std::io;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;

use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, FILE_FLAG_BACKUP_SEMANTICS, FILE_LIST_DIRECTORY, FILE_NOTIFY_CHANGE_DIR_NAME,
    FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING, ReadDirectoryChangesW,
};
use windows::core::PCWSTR;

/// Watches `dir` (not its subfolders) on a background thread and calls `on_change` after each
/// batch of folder changes. The thread ends when the folder goes away.
pub fn watch_folder(dir: &Path, on_change: impl Fn() + Send + 'static) -> io::Result<()> {
    let wide: Vec<u16> = dir.as_os_str().encode_wide().chain([0]).collect();
    let handle = unsafe {
        CreateFileW(
            PCWSTR(wide.as_ptr()),
            FILE_LIST_DIRECTORY.0,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            None,
            OPEN_EXISTING,
            FILE_FLAG_BACKUP_SEMANTICS,
            None,
        )
    }
    .map_err(io::Error::other)?;
    let raw = handle.0 as usize;
    std::thread::spawn(move || {
        let h = HANDLE(raw as *mut _);
        let mut buf = vec![0u8; 16 * 1024];
        loop {
            let mut returned = 0u32;
            let r = unsafe {
                ReadDirectoryChangesW(
                    h,
                    buf.as_mut_ptr().cast(),
                    buf.len() as u32,
                    false,
                    FILE_NOTIFY_CHANGE_DIR_NAME,
                    Some(&mut returned),
                    None,
                    None,
                )
            };
            if r.is_err() {
                break;
            }
            on_change();
        }
        let _ = unsafe { CloseHandle(h) };
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::time::Duration;

    #[test]
    fn reports_new_folders() {
        let dir = tempfile::tempdir().unwrap();
        let (tx, rx) = mpsc::channel();
        watch_folder(dir.path(), move || {
            let _ = tx.send(());
        })
        .unwrap();
        std::thread::sleep(Duration::from_millis(200));
        std::fs::create_dir(dir.path().join("New Game")).unwrap();
        assert!(rx.recv_timeout(Duration::from_secs(5)).is_ok());
    }
}
