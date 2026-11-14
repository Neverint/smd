//! Small JSON files in the app's local data folder (`%LOCALAPPDATA%\io.github.neverint.smd`).
//! Writes go to a temporary file first, then replace the old one, so a crash never leaves half a file.

use serde::Serialize;
use serde::de::DeserializeOwned;
use std::fs;
use std::io;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

pub fn read_json<T: DeserializeOwned>(path: &Path) -> io::Result<Option<T>> {
    match fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes).map(Some).map_err(|e| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("{}: {e}", path.display()),
            )
        }),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}

pub fn write_json<T: Serialize>(path: &Path, value: &T) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    // A temporary name of its own per write: two writes of the same file at once (two pages
    // asking for the drive audit together) must not rename each other's temporary file.
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let n = NEXT.fetch_add(1, Ordering::Relaxed);
    let tmp = path.with_extension(format!("json.{}-{n}.tmp", std::process::id()));
    let bytes = serde_json::to_vec_pretty(value).map_err(io::Error::other)?;
    fs::write(&tmp, bytes)?;
    fs::rename(&tmp, path).inspect_err(|_| {
        let _ = fs::remove_file(&tmp);
    })
}

pub fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_and_handles_missing_files() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sub/x.json");
        assert_eq!(read_json::<Vec<u32>>(&path).unwrap(), None);
        write_json(&path, &vec![1u32, 2, 3]).unwrap();
        assert_eq!(read_json::<Vec<u32>>(&path).unwrap(), Some(vec![1, 2, 3]));
        fs::write(&path, b"not json").unwrap();
        assert!(read_json::<Vec<u32>>(&path).is_err());
    }

    #[test]
    fn parallel_writes_of_one_file_all_succeed() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("policy.json");
        let handles: Vec<_> = (0..8u32)
            .map(|i| {
                let path = path.clone();
                std::thread::spawn(move || {
                    for _ in 0..50 {
                        write_json(&path, &vec![i; 100]).unwrap();
                    }
                })
            })
            .collect();
        for h in handles {
            h.join().unwrap();
        }
        let v = read_json::<Vec<u32>>(&path).unwrap().unwrap();
        assert_eq!(v.len(), 100);
        let leftovers = fs::read_dir(dir.path()).unwrap().count();
        assert_eq!(leftovers, 1, "no temporary files are left behind");
    }
}
