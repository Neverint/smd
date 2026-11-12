//! Quarantine on the disposable drive (S:): what the app removes from the other drives is
//! moved to `S:\Quarantine\<date>\<batch>\` instead of the Recycle Bin. That frees the space
//! on the source drive at once and keeps a 30-day safety net. Each batch has a manifest so
//! it can be put back exactly where it was.

use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::cleanup::{FailReason, Failure, Item, Outcome};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuarantinedItem {
    pub original: String,
    pub stored: String,
    pub bytes: u64,
    pub is_dir: bool,
    #[serde(default)]
    pub restored: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Batch {
    pub id: u64,
    pub created_ms: i64,
    pub items: Vec<QuarantinedItem>,
    /// Folder of the batch (not serialized in the manifest itself).
    #[serde(skip)]
    pub dir: PathBuf,
}

impl Batch {
    pub fn bytes(&self) -> u64 {
        self.items
            .iter()
            .filter(|i| !i.restored)
            .map(|i| i.bytes)
            .sum()
    }
}

const MANIFEST: &str = "manifest.json";

/// `S:\Quarantine` for a disposable drive letter like `S:`.
pub fn root_for(letter: &str) -> PathBuf {
    PathBuf::from(format!("{letter}\\Quarantine"))
}

fn same_volume(a: &Path, b: &Path) -> bool {
    let l = |p: &Path| {
        p.to_string_lossy()
            .get(0..2)
            .map(|s| s.to_ascii_uppercase())
    };
    l(a).is_some() && l(a) == l(b)
}

/// Moves a file or folder across volumes: copy, check sizes, then remove the original.
fn move_across(src: &Path, dst: &Path) -> io::Result<()> {
    if let Some(d) = dst.parent() {
        fs::create_dir_all(d)?;
    }
    if same_volume(src, dst) {
        return fs::rename(src, dst);
    }
    let meta = fs::symlink_metadata(src)?;
    if meta.file_type().is_symlink() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "links are not moved",
        ));
    }
    if meta.is_file() {
        fs::copy(src, dst)?;
        if fs::metadata(dst)?.len() != meta.len() {
            let _ = fs::remove_file(dst);
            return Err(io::Error::other("copy size mismatch"));
        }
        return fs::remove_file(src);
    }
    // Folder: copy everything first, check, then remove the original.
    let mut stack = vec![src.to_path_buf()];
    let mut total_src = 0u64;
    while let Some(dir) = stack.pop() {
        let rel = dir.strip_prefix(src).map_err(io::Error::other)?;
        fs::create_dir_all(dst.join(rel))?;
        for e in fs::read_dir(&dir)? {
            let e = e?;
            let ft = e.file_type()?;
            if ft.is_symlink() {
                continue;
            }
            if ft.is_dir() {
                stack.push(e.path());
            } else {
                let to = dst.join(e.path().strip_prefix(src).map_err(io::Error::other)?);
                fs::copy(e.path(), &to)?;
                total_src += e.metadata()?.len();
            }
        }
    }
    if crate::walk::measure(dst).bytes != total_src {
        let _ = fs::remove_dir_all(dst);
        return Err(io::Error::other("copy size mismatch"));
    }
    fs::remove_dir_all(src)
}

/// Puts items (already checked by `cleanup::remove_items`) into a new batch.
pub fn put(root: &Path, batch_id: u64, now_ms: i64, items: &[&Item]) -> (Outcome, Option<PathBuf>) {
    let dir = root
        .join(smd_core::format::iso_date(now_ms))
        .join(batch_id.to_string());
    let mut out = Outcome::default();
    let mut batch = Batch {
        id: batch_id,
        created_ms: now_ms,
        items: Vec::new(),
        dir: dir.clone(),
    };
    for item in items {
        let rel = item.path.replacen(':', "", 1);
        let stored = dir.join(&rel);
        match move_across(Path::new(&item.path), &stored) {
            Ok(()) => {
                out.removed.push((item.path.clone(), item.bytes));
                batch.items.push(QuarantinedItem {
                    original: item.path.clone(),
                    stored: stored.to_string_lossy().into_owned(),
                    bytes: item.bytes,
                    is_dir: item.is_dir,
                    restored: false,
                });
            }
            Err(_) => out.failed.push(Failure {
                path: item.path.clone(),
                reason: FailReason::InUse,
            }),
        }
    }
    if batch.items.is_empty() {
        let _ = fs::remove_dir_all(&dir);
        return (out, None);
    }
    let manifest = dir.join(MANIFEST);
    if fs::write(
        &manifest,
        serde_json::to_vec_pretty(&batch).unwrap_or_default(),
    )
    .is_err()
    {
        log::warn!(
            "could not write the quarantine manifest {}",
            manifest.display()
        );
    }
    (out, Some(manifest))
}

pub fn load(manifest: &Path) -> io::Result<Batch> {
    let mut b: Batch = serde_json::from_slice(&fs::read(manifest)?)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    b.dir = manifest.parent().map(Path::to_path_buf).unwrap_or_default();
    Ok(b)
}

/// All batches, newest first.
pub fn list(root: &Path) -> Vec<Batch> {
    let mut out = Vec::new();
    let Ok(days) = fs::read_dir(root) else {
        return out;
    };
    for d in days.flatten() {
        let Ok(batches) = fs::read_dir(d.path()) else {
            continue;
        };
        for b in batches.flatten() {
            if let Ok(batch) = load(&b.path().join(MANIFEST))
                && batch.items.iter().any(|i| !i.restored)
            {
                out.push(batch);
            }
        }
    }
    out.sort_by_key(|b| std::cmp::Reverse(b.created_ms));
    out
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuarantineRestore {
    pub restored: u64,
    /// Something is already back at the original place; the quarantined copy was kept.
    pub conflicts: u64,
    pub failed: u64,
}

/// Puts a batch back where it came from. Never overwrites.
pub fn restore(manifest: &Path) -> io::Result<QuarantineRestore> {
    let mut b = load(manifest)?;
    let mut r = QuarantineRestore::default();
    for item in b.items.iter_mut().filter(|i| !i.restored) {
        let original = Path::new(&item.original);
        if original.exists() {
            r.conflicts += 1;
            continue;
        }
        match move_across(Path::new(&item.stored), original) {
            Ok(()) => {
                item.restored = true;
                r.restored += 1;
            }
            Err(_) => r.failed += 1,
        }
    }
    fs::write(
        manifest,
        serde_json::to_vec_pretty(&b).map_err(io::Error::other)?,
    )?;
    if b.items.iter().all(|i| i.restored) {
        let _ = fs::remove_dir_all(&b.dir);
    }
    Ok(r)
}

/// Deletes batches older than `days` for good. Returns the bytes freed.
pub fn purge(root: &Path, days: u32, now_ms: i64) -> u64 {
    let cutoff = now_ms - i64::from(days) * 86_400_000;
    let mut freed = 0;
    for b in list(root).into_iter().filter(|b| b.created_ms < cutoff) {
        let bytes = b.bytes();
        // Only folders inside the quarantine root, checked like any other removal.
        if smd_core::safety::is_inside(&b.dir.to_string_lossy(), &root.to_string_lossy())
            && fs::remove_dir_all(&b.dir).is_ok()
        {
            freed += bytes;
        }
    }
    // Remove empty day folders.
    if let Ok(days) = fs::read_dir(root) {
        for d in days.flatten() {
            let _ = fs::remove_dir(d.path());
        }
    }
    freed
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(path: &Path, bytes: u64, is_dir: bool) -> Item {
        Item {
            path: path.to_string_lossy().into_owned(),
            name: String::new(),
            bytes,
            files: 1,
            modified_ms: None,
            is_dir,
            root: String::new(),
            item_is_root: false,
        }
    }

    /// Across drives when the system temp folder and the repository are on different drives.
    #[test]
    fn moves_folders_across_volumes() {
        let a = tempfile::tempdir().unwrap();
        let b = tempfile::tempdir_in(env!("CARGO_MANIFEST_DIR")).unwrap();
        let src = a.path().join("folder");
        fs::create_dir_all(src.join("deep")).unwrap();
        fs::write(src.join(r"deep\f.bin"), vec![9u8; 70_000]).unwrap();
        let dst = b.path().join(r"Q\C\folder");
        move_across(&src, &dst).unwrap();
        assert!(!src.exists());
        assert_eq!(fs::read(dst.join(r"deep\f.bin")).unwrap().len(), 70_000);
    }

    #[test]
    fn quarantines_restores_and_purges() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("src");
        fs::create_dir_all(src.join(r"cache\sub")).unwrap();
        fs::write(src.join("junk.tmp"), b"12345").unwrap();
        fs::write(src.join(r"cache\sub\x.bin"), b"abc").unwrap();
        let root = dir.path().join("Quarantine");
        let items = [
            item(&src.join("junk.tmp"), 5, false),
            item(&src.join("cache"), 3, true),
        ];
        let refs: Vec<&Item> = items.iter().collect();

        let (out, manifest) = put(&root, 7, 1_794_441_600_000, &refs);
        assert_eq!(out.removed.len(), 2);
        assert!(!src.join("junk.tmp").exists() && !src.join("cache").exists());
        let manifest = manifest.unwrap();
        assert!(manifest.to_string_lossy().contains("2026-11-12"));
        let batches = list(&root);
        assert_eq!(batches.len(), 1);
        assert_eq!(batches[0].bytes(), 8);

        // Something new took one of the names: that item stays in quarantine.
        fs::write(src.join("junk.tmp"), b"new").unwrap();
        let r = restore(&manifest).unwrap();
        assert_eq!(
            r,
            QuarantineRestore {
                restored: 1,
                conflicts: 1,
                failed: 0
            }
        );
        assert_eq!(fs::read(src.join(r"cache\sub\x.bin")).unwrap(), b"abc");
        assert_eq!(
            fs::read(src.join("junk.tmp")).unwrap(),
            b"new",
            "never overwritten"
        );

        assert_eq!(
            purge(&root, 30, 1_794_441_600_000 + 10 * 86_400_000),
            0,
            "not old enough"
        );
        assert_eq!(purge(&root, 30, 1_794_441_600_000 + 31 * 86_400_000), 5);
        assert!(list(&root).is_empty());
    }
}
