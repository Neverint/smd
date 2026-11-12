//! My Clutter: large files, old files and duplicates in your own folders. Read-only.
//!
//! Duplicates are found in three steps: same size, then the same hash of the first and
//! last 64 KB, then the same full BLAKE3 hash. Hard links (one file under two names)
//! are not duplicates and are left out.

use rayon::prelude::*;
use serde::Serialize;
use std::collections::HashMap;
use std::fs;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

#[derive(Debug, Clone, Copy)]
pub struct ClutterOptions {
    /// "Large": at least this size.
    pub large_min: u64,
    /// "Old": at least this size and not changed for `old_days`.
    pub old_min: u64,
    pub old_days: u32,
    /// Duplicates: files at least this size.
    pub duplicate_min: u64,
    /// At most this many entries per list.
    pub max_list: usize,
}

impl Default for ClutterOptions {
    fn default() -> Self {
        ClutterOptions {
            large_min: 100 * 1024 * 1024,
            old_min: 10 * 1024 * 1024,
            old_days: 365,
            duplicate_min: 1024 * 1024,
            max_list: 500,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileEntry {
    pub path: String,
    pub name: String,
    pub bytes: u64,
    pub modified_ms: Option<i64>,
    /// The scanned folder it was found in (checked again before removal).
    #[serde(skip)]
    pub root: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateGroup {
    /// Size of each copy.
    pub bytes: u64,
    pub files: Vec<FileEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClutterResult {
    pub large: Vec<FileEntry>,
    pub old: Vec<FileEntry>,
    pub duplicates: Vec<DuplicateGroup>,
    /// Space the extra copies use (all but one per group).
    pub duplicate_bytes: u64,
    pub files_scanned: u64,
    pub bytes_scanned: u64,
    pub cancelled: bool,
}

#[derive(Default)]
pub struct ClutterProgress {
    /// 0 = walking, 1 = comparing duplicates
    pub stage: AtomicU64,
    pub files: AtomicU64,
    pub bytes: AtomicU64,
    /// Bytes hashed so far / to hash (stage 1).
    pub hashed: AtomicU64,
    pub to_hash: AtomicU64,
}

/// Folder names never walked: system folders and app data. Your own files only.
const SKIP_DIRS: &[&str] = &[
    "windows",
    "program files",
    "program files (x86)",
    "programdata",
    "$recycle.bin",
    "system volume information",
    "appdata",
    "$winreagent",
    "recovery",
    "$windows.~bt",
    "$windows.~ws",
    "windowsapps",
];

fn modified_ms(m: &fs::Metadata) -> Option<i64> {
    m.modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as i64)
}

fn collect(
    dir: &Path,
    root: &str,
    min: u64,
    out: &Mutex<Vec<FileEntry>>,
    p: &ClutterProgress,
    cancel: &AtomicBool,
) {
    if cancel.load(Ordering::Relaxed) {
        return;
    }
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let mut subdirs = Vec::new();
    let mut local = Vec::new();
    for e in entries.flatten() {
        let Ok(ft) = e.file_type() else { continue };
        if ft.is_symlink() {
            continue;
        }
        let name = e.file_name().to_string_lossy().into_owned();
        if ft.is_dir() {
            if !SKIP_DIRS.contains(&name.to_lowercase().as_str()) && !is_hidden_system(&e) {
                subdirs.push(e.path());
            }
            continue;
        }
        let Ok(meta) = e.metadata() else { continue };
        p.files.fetch_add(1, Ordering::Relaxed);
        p.bytes.fetch_add(meta.len(), Ordering::Relaxed);
        if meta.len() >= min {
            local.push(FileEntry {
                path: e.path().to_string_lossy().into_owned(),
                name,
                bytes: meta.len(),
                modified_ms: modified_ms(&meta),
                root: root.to_string(),
            });
        }
    }
    if !local.is_empty()
        && let Ok(mut o) = out.lock()
    {
        o.extend(local);
    }
    subdirs
        .par_iter()
        .for_each(|d| collect(d, root, min, out, p, cancel));
}

#[cfg(windows)]
fn is_hidden_system(e: &fs::DirEntry) -> bool {
    use std::os::windows::fs::MetadataExt;
    const HIDDEN: u32 = 0x2;
    const SYSTEM: u32 = 0x4;
    e.metadata()
        .is_ok_and(|m| m.file_attributes() & (HIDDEN | SYSTEM) == (HIDDEN | SYSTEM))
}

#[cfg(not(windows))]
fn is_hidden_system(_e: &fs::DirEntry) -> bool {
    false
}

/// Volume serial + file index: two names for the same file share it.
#[cfg(windows)]
fn file_identity(path: &str) -> Option<(u32, u64)> {
    use std::os::windows::io::AsRawHandle;
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::Storage::FileSystem::{
        BY_HANDLE_FILE_INFORMATION, GetFileInformationByHandle,
    };
    let f = fs::File::open(path).ok()?;
    let mut info = BY_HANDLE_FILE_INFORMATION::default();
    unsafe { GetFileInformationByHandle(HANDLE(f.as_raw_handle()), &mut info) }.ok()?;
    Some((
        info.dwVolumeSerialNumber,
        (u64::from(info.nFileIndexHigh) << 32) | u64::from(info.nFileIndexLow),
    ))
}

#[cfg(not(windows))]
fn file_identity(_path: &str) -> Option<(u32, u64)> {
    None
}

const EDGE: usize = 64 * 1024;

fn partial_hash(path: &str, size: u64) -> io::Result<[u8; 32]> {
    let mut f = fs::File::open(path)?;
    let mut h = blake3::Hasher::new();
    let mut buf = vec![0u8; EDGE];
    let n = f.read(&mut buf)?;
    h.update(&buf[..n]);
    if size > (EDGE * 2) as u64 {
        f.seek(SeekFrom::End(-(EDGE as i64)))?;
        let n = f.read(&mut buf)?;
        h.update(&buf[..n]);
    }
    Ok(*h.finalize().as_bytes())
}

fn full_hash(path: &str, p: &ClutterProgress, cancel: &AtomicBool) -> io::Result<[u8; 32]> {
    let mut f = fs::File::open(path)?;
    let mut h = blake3::Hasher::new();
    let mut buf = vec![0u8; 1024 * 1024];
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Err(io::Error::new(io::ErrorKind::Interrupted, "cancelled"));
        }
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
        p.hashed.fetch_add(n as u64, Ordering::Relaxed);
    }
    Ok(*h.finalize().as_bytes())
}

/// Splits each group by `key`, keeping only sub-groups with two or more files.
fn refine<K: std::hash::Hash + Eq + Send>(
    groups: Vec<Vec<FileEntry>>,
    key: impl Fn(&FileEntry) -> Option<K> + Sync,
) -> Vec<Vec<FileEntry>> {
    groups
        .into_par_iter()
        .flat_map(|g| {
            let mut by: HashMap<K, Vec<FileEntry>> = HashMap::new();
            for f in g {
                if let Some(k) = key(&f) {
                    by.entry(k).or_default().push(f);
                }
            }
            by.into_values().filter(|v| v.len() > 1).collect::<Vec<_>>()
        })
        .collect()
}

/// Groups identical files. Hard links count as one file.
pub fn find_duplicates(
    files: Vec<FileEntry>,
    p: &ClutterProgress,
    cancel: &AtomicBool,
) -> Vec<DuplicateGroup> {
    let mut by_size: HashMap<u64, Vec<FileEntry>> = HashMap::new();
    for f in files {
        by_size.entry(f.bytes).or_default().push(f);
    }
    let groups: Vec<Vec<FileEntry>> = by_size.into_values().filter(|v| v.len() > 1).collect();

    // Hard links: keep one name per physical file.
    let groups: Vec<Vec<FileEntry>> = groups
        .into_par_iter()
        .filter_map(|g| {
            let mut seen = std::collections::HashSet::new();
            let g: Vec<FileEntry> = g
                .into_iter()
                .filter(|f| file_identity(&f.path).is_none_or(|id| seen.insert(id)))
                .collect();
            (g.len() > 1).then_some(g)
        })
        .collect();
    let groups = refine(groups, |f| partial_hash(&f.path, f.bytes).ok());
    p.to_hash.store(
        groups.iter().flatten().map(|f| f.bytes).sum(),
        Ordering::Relaxed,
    );
    p.stage.store(1, Ordering::Relaxed);
    let groups = refine(groups, |f| full_hash(&f.path, p, cancel).ok());

    let mut out: Vec<DuplicateGroup> = groups
        .into_iter()
        .map(|mut files| {
            files.sort_by(|a, b| a.path.len().cmp(&b.path.len()).then(a.path.cmp(&b.path)));
            DuplicateGroup {
                bytes: files[0].bytes,
                files,
            }
        })
        .collect();
    out.sort_by_key(|g| std::cmp::Reverse(g.bytes * (g.files.len() as u64 - 1)));
    out
}

/// Scans `roots` for large, old and duplicate files.
pub fn scan(
    roots: &[PathBuf],
    opts: ClutterOptions,
    now_ms: i64,
    p: &ClutterProgress,
    cancel: &AtomicBool,
) -> ClutterResult {
    let min = opts.large_min.min(opts.old_min).min(opts.duplicate_min);
    let found = Mutex::new(Vec::new());
    for root in roots {
        collect(root, &root.to_string_lossy(), min, &found, p, cancel);
    }
    let mut files = found.into_inner().unwrap_or_default();
    // Overlapping roots (Documents inside Desktop...) list a file once.
    files.sort_by_key(|a| a.path.to_lowercase());
    files.dedup_by(|a, b| a.path.eq_ignore_ascii_case(&b.path));

    let mut large: Vec<FileEntry> = files
        .iter()
        .filter(|f| f.bytes >= opts.large_min)
        .cloned()
        .collect();
    large.sort_by_key(|f| std::cmp::Reverse(f.bytes));
    large.truncate(opts.max_list);

    let cutoff = now_ms - i64::from(opts.old_days) * 86_400_000;
    let mut old: Vec<FileEntry> = files
        .iter()
        .filter(|f| f.bytes >= opts.old_min && f.modified_ms.is_some_and(|m| m < cutoff))
        .cloned()
        .collect();
    old.sort_by_key(|f| std::cmp::Reverse(f.bytes));
    old.truncate(opts.max_list);

    let dup_candidates: Vec<FileEntry> = files
        .into_iter()
        .filter(|f| f.bytes >= opts.duplicate_min)
        .collect();
    let mut duplicates = if cancel.load(Ordering::Relaxed) {
        Vec::new()
    } else {
        find_duplicates(dup_candidates, p, cancel)
    };
    duplicates.truncate(opts.max_list);
    let duplicate_bytes = duplicates
        .iter()
        .map(|g| g.bytes * (g.files.len() as u64 - 1))
        .sum();

    ClutterResult {
        large,
        old,
        duplicates,
        duplicate_bytes,
        files_scanned: p.files.load(Ordering::Relaxed),
        bytes_scanned: p.bytes.load(Ordering::Relaxed),
        cancelled: cancel.load(Ordering::Relaxed),
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderItem {
    pub path: String,
    pub name: String,
    pub bytes: u64,
    pub files: u64,
    pub modified_ms: Option<i64>,
    pub is_dir: bool,
}

/// Everything directly inside a folder (Downloads), measured. Read-only.
pub fn folder_items(dir: &Path) -> io::Result<Vec<FolderItem>> {
    let entries: Vec<fs::DirEntry> = fs::read_dir(dir)?.flatten().collect();
    let mut out: Vec<FolderItem> = entries
        .par_iter()
        .filter(|e| !e.file_type().is_ok_and(|t| t.is_symlink()))
        .filter(|e| {
            !e.file_name()
                .to_string_lossy()
                .eq_ignore_ascii_case("desktop.ini")
        })
        .map(|e| {
            let m = crate::walk::measure(&e.path());
            let is_dir = e.file_type().is_ok_and(|t| t.is_dir());
            let own = e.metadata().ok().and_then(|m| modified_ms(&m));
            FolderItem {
                path: e.path().to_string_lossy().into_owned(),
                name: e.file_name().to_string_lossy().into_owned(),
                bytes: m.bytes,
                files: m.files,
                modified_ms: if is_dir { m.newest_ms } else { own },
                is_dir,
            }
        })
        .collect();
    out.sort_by_key(|f| std::cmp::Reverse(f.bytes));
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write(path: &Path, bytes: &[u8]) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::File::create(path).unwrap().write_all(bytes).unwrap();
    }

    fn opts() -> ClutterOptions {
        ClutterOptions {
            large_min: 300_000,
            old_min: 1000,
            old_days: 365,
            duplicate_min: 10,
            max_list: 100,
        }
    }

    #[test]
    fn finds_large_old_and_duplicate_files() {
        let dir = tempfile::tempdir().unwrap();
        let r = dir.path();
        let big: Vec<u8> = (0..400_000u32).map(|i| (i % 251) as u8).collect();
        write(&r.join("a/big.bin"), &big);
        write(&r.join("b/copy-of-big.bin"), &big);
        let mut almost = big.clone();
        almost[200_000] ^= 1; // same size, same first and last 64 KB, different middle
        write(&r.join("c/almost.bin"), &almost);
        write(&r.join("small1.txt"), b"hello world, same text");
        write(&r.join("d/small2.txt"), b"hello world, same text");
        write(&r.join("AppData/skipped.bin"), &big);

        let p = ClutterProgress::default();
        let res = scan(
            &[r.to_path_buf()],
            opts(),
            i64::MAX / 4,
            &p,
            &AtomicBool::new(false),
        );
        assert!(!res.cancelled);
        assert_eq!(res.large.len(), 3, "AppData is never walked");
        assert_eq!(res.duplicates.len(), 2);
        let big_group = &res.duplicates[0];
        assert_eq!(big_group.bytes, 400_000);
        assert_eq!(
            big_group.files.len(),
            2,
            "the file that differs in the middle is not a duplicate"
        );
        assert_eq!(res.duplicate_bytes, 400_000 + 22);
        assert!(res.old.iter().all(|f| f.bytes >= 1000));
        assert!(p.hashed.load(Ordering::Relaxed) > 0);
    }

    #[cfg(windows)]
    #[test]
    fn hard_links_are_not_duplicates() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.bin");
        write(&a, &[7u8; 5000]);
        fs::hard_link(&a, dir.path().join("b.bin")).unwrap();
        let res = scan(
            &[dir.path().to_path_buf()],
            opts(),
            0,
            &ClutterProgress::default(),
            &AtomicBool::new(false),
        );
        assert!(res.duplicates.is_empty());
    }

    #[test]
    fn lists_folder_items() {
        let dir = tempfile::tempdir().unwrap();
        write(&dir.path().join("setup.exe"), &[1u8; 300]);
        write(&dir.path().join("photos/x.jpg"), &[1u8; 500]);
        let items = folder_items(dir.path()).unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].name, "photos");
        assert!(items[0].is_dir && items[0].files == 1);
    }
}
