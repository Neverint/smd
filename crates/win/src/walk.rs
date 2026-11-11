//! Parallel, read-only disk walk for Space Lens.
//!
//! - Symlinks, junctions and mount points are never followed (no loops, no double counting).
//! - Folders Windows refuses to list are marked `denied` and counted, never reported as errors.
//! - Sizes come from the directory listing (on Windows, `DirEntry::metadata` reuses the data
//!   from `FindNextFileW`, so no file is opened).
//! - To bound memory on a 1 TB drive, a folder keeps its largest files (at least `keep_min_bytes`,
//!   at most `keep_per_dir`); the rest are grouped into one "small files" node.

use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use rayon::prelude::*;
use smd_core::space::{ExtStat, NodeKind, RawNode};

#[derive(Debug, Clone, Copy)]
pub struct WalkOptions {
    pub keep_min_bytes: u64,
    pub keep_per_dir: usize,
}

impl Default for WalkOptions {
    fn default() -> Self {
        WalkOptions {
            keep_min_bytes: 1024 * 1024,
            keep_per_dir: 40,
        }
    }
}

/// Live counters, read by the UI while the walk runs.
#[derive(Default)]
pub struct Progress {
    pub files: AtomicU64,
    pub dirs: AtomicU64,
    pub bytes: AtomicU64,
    pub denied: AtomicU64,
    current: Mutex<String>,
}

impl Progress {
    pub fn current(&self) -> String {
        self.current.lock().map(|c| c.clone()).unwrap_or_default()
    }
    fn set_current(&self, path: &Path) {
        if let Ok(mut c) = self.current.try_lock() {
            *c = path.to_string_lossy().into_owned();
        }
    }
}

pub struct WalkResult {
    pub tree: RawNode,
    pub extensions: Vec<ExtStat>,
    pub cancelled: bool,
}

/// Walks `root`. Returns early (with `cancelled`) once `cancel` is set.
pub fn walk(
    root: &Path,
    opts: WalkOptions,
    progress: &Progress,
    cancel: &AtomicBool,
) -> WalkResult {
    let exts: Mutex<HashMap<String, (u64, u64)>> = Mutex::new(HashMap::new());
    let ctx = Ctx {
        opts,
        progress,
        cancel,
        exts: &exts,
    };
    // Deep folder trees recurse through rayon; give the workers room.
    let tree = match rayon::ThreadPoolBuilder::new()
        .stack_size(16 * 1024 * 1024)
        .build()
    {
        Ok(pool) => pool.install(|| walk_dir(root, String::new(), &ctx)),
        Err(_) => walk_dir(root, String::new(), &ctx),
    };
    let extensions = exts
        .into_inner()
        .unwrap_or_default()
        .into_iter()
        .map(|(ext, (bytes, files))| ExtStat { ext, bytes, files })
        .collect();
    WalkResult {
        tree,
        extensions,
        cancelled: cancel.load(Ordering::Relaxed),
    }
}

struct Ctx<'a> {
    opts: WalkOptions,
    progress: &'a Progress,
    cancel: &'a AtomicBool,
    exts: &'a Mutex<HashMap<String, (u64, u64)>>,
}

fn walk_dir(path: &Path, name: String, ctx: &Ctx<'_>) -> RawNode {
    let mut node = RawNode {
        name,
        kind: NodeKind::Dir,
        size: 0,
        files: 0,
        denied: false,
        children: Vec::new(),
    };
    if ctx.cancel.load(Ordering::Relaxed) {
        return node;
    }
    ctx.progress.dirs.fetch_add(1, Ordering::Relaxed);
    ctx.progress.set_current(path);

    let entries = match fs::read_dir(path) {
        Ok(e) => e,
        Err(_) => {
            node.denied = true;
            ctx.progress.denied.fetch_add(1, Ordering::Relaxed);
            return node;
        }
    };

    let mut subdirs = Vec::new();
    let mut files: Vec<RawNode> = Vec::new();
    let mut local_exts: HashMap<String, (u64, u64)> = HashMap::new();
    let (mut small_count, mut small_size) = (0u64, 0u64);

    for entry in entries.flatten() {
        let Ok(ft) = entry.file_type() else { continue };
        // On Windows this is true for symlinks, junctions and volume mount points.
        // Other reparse folders (OneDrive placeholders) are real folders and are walked.
        if ft.is_symlink() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        if ft.is_dir() {
            subdirs.push((entry.path(), name));
            continue;
        }
        let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
        ctx.progress.files.fetch_add(1, Ordering::Relaxed);
        ctx.progress.bytes.fetch_add(size, Ordering::Relaxed);
        let ext = Path::new(&name)
            .extension()
            .map(|e| e.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        let slot = local_exts.entry(ext).or_default();
        slot.0 += size;
        slot.1 += 1;
        if size >= ctx.opts.keep_min_bytes {
            files.push(RawNode::file(name, size));
        } else {
            small_count += 1;
            small_size += size;
        }
    }

    if let Ok(mut all) = ctx.exts.lock() {
        for (ext, (b, f)) in local_exts {
            let slot = all.entry(ext).or_default();
            slot.0 += b;
            slot.1 += f;
        }
    }

    if files.len() > ctx.opts.keep_per_dir {
        files.sort_unstable_by_key(|f| std::cmp::Reverse(f.size));
        for f in files.split_off(ctx.opts.keep_per_dir) {
            small_count += 1;
            small_size += f.size;
        }
    }

    let dirs: Vec<RawNode> = subdirs
        .into_par_iter()
        .map(|(p, n)| walk_dir(&p, n, ctx))
        .collect();

    node.children = dirs;
    node.children.extend(files);
    if small_count > 0 {
        node.children.push(RawNode::others(small_count, small_size));
    }
    node.size = node.children.iter().map(|c| c.size).sum();
    node.files = node.children.iter().map(|c| c.files).sum();
    node
}

/// Size, file count and newest modification time (ms since 1970) of a file or folder,
/// without following links. Unreadable parts are skipped.
pub fn measure(path: &Path) -> Measure {
    let mut m = Measure::default();
    measure_into(path, &mut m, 0);
    m
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Measure {
    pub bytes: u64,
    pub files: u64,
    pub newest_ms: Option<i64>,
    pub unreadable: bool,
}

fn modified_ms(meta: &fs::Metadata) -> Option<i64> {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as i64)
}

fn measure_into(path: &Path, m: &mut Measure, depth: u32) {
    let Ok(meta) = fs::symlink_metadata(path) else {
        m.unreadable = true;
        return;
    };
    if meta.file_type().is_symlink() {
        return;
    }
    let mtime = modified_ms(&meta);
    if !meta.is_dir() {
        m.bytes += meta.len();
        m.files += 1;
        m.newest_ms = m.newest_ms.max(mtime);
        return;
    }
    if depth == 0 {
        m.newest_ms = m.newest_ms.max(mtime);
    }
    let Ok(entries) = fs::read_dir(path) else {
        m.unreadable = true;
        return;
    };
    for entry in entries.flatten() {
        let Ok(ft) = entry.file_type() else { continue };
        if ft.is_symlink() {
            continue;
        }
        if ft.is_dir() {
            measure_into(&entry.path(), m, depth + 1);
        } else if let Ok(meta) = entry.metadata() {
            m.bytes += meta.len();
            m.files += 1;
            m.newest_ms = m.newest_ms.max(modified_ms(&meta));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write(path: &Path, bytes: usize) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let mut f = fs::File::create(path).unwrap();
        f.write_all(&vec![7u8; bytes]).unwrap();
    }

    #[test]
    fn walks_and_groups_small_files() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write(&root.join("big.bin"), 4096);
        write(&root.join("a.txt"), 10);
        write(&root.join("b.txt"), 20);
        write(&root.join("sub/deeper/c.iso"), 8192);

        let opts = WalkOptions {
            keep_min_bytes: 1000,
            keep_per_dir: 10,
        };
        let progress = Progress::default();
        let r = walk(root, opts, &progress, &AtomicBool::new(false));
        assert!(!r.cancelled);
        assert_eq!(r.tree.size, 4096 + 10 + 20 + 8192);
        assert_eq!(r.tree.files, 4);
        assert_eq!(progress.files.load(Ordering::Relaxed), 4);
        assert_eq!(progress.dirs.load(Ordering::Relaxed), 3);

        let others = r
            .tree
            .children
            .iter()
            .find(|c| c.kind == NodeKind::Others)
            .unwrap();
        assert_eq!((others.files, others.size), (2, 30));
        assert!(
            r.tree
                .children
                .iter()
                .any(|c| c.name == "big.bin" && c.kind == NodeKind::File)
        );
        let sub = r.tree.children.iter().find(|c| c.name == "sub").unwrap();
        assert_eq!(sub.size, 8192);

        let txt = r.extensions.iter().find(|e| e.ext == "txt").unwrap();
        assert_eq!((txt.bytes, txt.files), (30, 2));
    }

    #[test]
    fn keeps_only_the_largest_files_per_folder() {
        let dir = tempfile::tempdir().unwrap();
        for i in 1..=5 {
            write(&dir.path().join(format!("f{i}.bin")), i * 100);
        }
        let opts = WalkOptions {
            keep_min_bytes: 0,
            keep_per_dir: 2,
        };
        let r = walk(
            dir.path(),
            opts,
            &Progress::default(),
            &AtomicBool::new(false),
        );
        let kept: Vec<_> = r
            .tree
            .children
            .iter()
            .filter(|c| c.kind == NodeKind::File)
            .map(|c| c.size)
            .collect();
        assert_eq!(kept.len(), 2);
        assert!(kept.contains(&500) && kept.contains(&400));
        let others = r
            .tree
            .children
            .iter()
            .find(|c| c.kind == NodeKind::Others)
            .unwrap();
        assert_eq!((others.files, others.size), (3, 600));
        assert_eq!(r.tree.size, 1500);
    }

    #[test]
    fn stops_when_cancelled() {
        let dir = tempfile::tempdir().unwrap();
        write(&dir.path().join("x/y.bin"), 100);
        let r = walk(
            dir.path(),
            WalkOptions::default(),
            &Progress::default(),
            &AtomicBool::new(true),
        );
        assert!(r.cancelled);
        assert_eq!(r.tree.size, 0);
    }

    #[test]
    fn measures_files_and_folders() {
        let dir = tempfile::tempdir().unwrap();
        write(&dir.path().join("x/a.bin"), 100);
        write(&dir.path().join("x/y/b.bin"), 50);
        let m = measure(&dir.path().join("x"));
        assert_eq!((m.bytes, m.files), (150, 2));
        assert!(m.newest_ms.is_some());
        assert!(!m.unreadable);
        let missing = measure(&dir.path().join("nope"));
        assert!(missing.unreadable);
    }

    #[cfg(windows)]
    #[test]
    fn does_not_follow_junctions() {
        let dir = tempfile::tempdir().unwrap();
        write(&dir.path().join("target/data.bin"), 1000);
        let link = dir.path().join("link");
        let status = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(&link)
            .arg(dir.path().join("target"))
            .output()
            .unwrap();
        assert!(status.status.success(), "mklink /J failed");
        let r = walk(
            dir.path(),
            WalkOptions::default(),
            &Progress::default(),
            &AtomicBool::new(false),
        );
        assert_eq!(r.tree.size, 1000, "the junction must not be counted twice");
        assert_eq!(measure(dir.path()).bytes, 1000);
    }
}
