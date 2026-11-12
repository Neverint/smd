//! Migrations: copy, verify (size + BLAKE3), then move the originals to their drive's
//! Recycle Bin. Every step is written to a journal, so a crash or a reboot never loses data
//! and the move can be resumed. Junctions and symlinks are never followed or copied.

use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StepState {
    Pending,
    Copying,
    /// Copied and verified; the original is still in place.
    Verified,
    /// Copied, verified, original moved to the Recycle Bin.
    Done,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvChange {
    pub name: String,
    pub old: Option<String>,
    pub new: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JournalFile {
    /// Relative to the step's source (empty when the source is a single file).
    pub rel: String,
    pub bytes: u64,
    pub verified: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JournalStep {
    pub finding_id: String,
    pub name: String,
    pub source: String,
    pub target: String,
    pub bytes: u64,
    /// For tool caches: the environment variable pointed to the new place.
    #[serde(default)]
    pub env_var: Option<String>,
    #[serde(default)]
    pub env_change: Option<EnvChange>,
    pub state: StepState,
    #[serde(default)]
    pub error: Option<String>,
    /// Filled when copying starts.
    #[serde(default)]
    pub files: Option<Vec<JournalFile>>,
    /// Links (junctions, symlinks) found in the source, left in place.
    #[serde(default)]
    pub skipped_links: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Journal {
    pub id: u64,
    pub created_ms: i64,
    pub steps: Vec<JournalStep>,
    /// All steps are Done or Failed.
    #[serde(default)]
    pub finished: bool,
}

impl Journal {
    pub fn load(path: &Path) -> io::Result<Journal> {
        serde_json::from_slice(&fs::read(path)?)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }

    pub fn save(&self, path: &Path) -> io::Result<()> {
        if let Some(d) = path.parent() {
            fs::create_dir_all(d)?;
        }
        let tmp = path.with_extension("json.tmp");
        fs::write(
            &tmp,
            serde_json::to_vec_pretty(self).map_err(io::Error::other)?,
        )?;
        fs::rename(&tmp, path)
    }
}

#[derive(Default)]
pub struct MigrateProgress {
    pub step: AtomicU64,
    pub bytes_done: AtomicU64,
    pub bytes_total: AtomicU64,
}

fn hash_file(p: &Path) -> io::Result<[u8; 32]> {
    let mut f = fs::File::open(p)?;
    let mut h = blake3::Hasher::new();
    let mut buf = vec![0u8; 1024 * 1024];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(*h.finalize().as_bytes())
}

/// Lists the files to copy. Links are skipped and counted.
fn list_files(source: &Path) -> io::Result<(Vec<JournalFile>, u32)> {
    let meta = fs::symlink_metadata(source)?;
    if meta.file_type().is_symlink() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "the source is a link",
        ));
    }
    if meta.is_file() {
        return Ok((
            vec![JournalFile {
                rel: String::new(),
                bytes: meta.len(),
                verified: false,
            }],
            0,
        ));
    }
    let mut files = Vec::new();
    let mut links = 0;
    let mut stack = vec![source.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for e in fs::read_dir(&dir)? {
            let e = e?;
            let ft = e.file_type()?;
            if ft.is_symlink() {
                links += 1;
            } else if ft.is_dir() {
                stack.push(e.path());
            } else {
                let rel = e
                    .path()
                    .strip_prefix(source)
                    .map_err(io::Error::other)?
                    .to_string_lossy()
                    .into_owned();
                files.push(JournalFile {
                    rel,
                    bytes: e.metadata()?.len(),
                    verified: false,
                });
            }
        }
    }
    Ok((files, links))
}

fn join(base: &Path, rel: &str) -> PathBuf {
    if rel.is_empty() {
        base.to_path_buf()
    } else {
        base.join(rel)
    }
}

/// Copies one file (keeping its timestamps and attributes) and checks size and content.
fn copy_verified(src: &Path, dst: &Path) -> io::Result<()> {
    if let Some(d) = dst.parent() {
        fs::create_dir_all(d)?;
    }
    let part = PathBuf::from(format!("{}.smdpart", dst.display()));
    // CopyFileExW: preserves timestamps and attributes.
    fs::copy(src, &part)?;
    let (a, b) = (fs::metadata(src)?.len(), fs::metadata(&part)?.len());
    if a != b || hash_file(src)? != hash_file(&part)? {
        let _ = fs::remove_file(&part);
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("copy of {} does not match the original", src.display()),
        ));
    }
    fs::rename(&part, dst)
}

/// Hooks the mover needs from the rest of the app (Recycle Bin, environment variables).
pub trait Effects {
    fn recycle(&self, path: &Path) -> io::Result<()>;
    fn get_env(&self, name: &str) -> Option<String>;
    fn set_env(&self, name: &str, value: Option<&str>) -> io::Result<()>;
}

/// The real thing: the Recycle Bin and user environment variables.
pub struct WindowsEffects;

impl Effects for WindowsEffects {
    fn recycle(&self, path: &Path) -> io::Result<()> {
        trash::delete(path).map_err(io::Error::other)?;
        if path.exists() {
            Err(io::Error::other("the original is still in use"))
        } else {
            Ok(())
        }
    }
    fn get_env(&self, name: &str) -> Option<String> {
        crate::env::get_user(name)
    }
    fn set_env(&self, name: &str, value: Option<&str>) -> io::Result<()> {
        crate::env::set_user(name, value)
    }
}

/// Runs (or resumes) a journal. Stops between files when `cancel` is set; run it again to continue.
pub fn run(
    journal_path: &Path,
    fx: &dyn Effects,
    progress: &MigrateProgress,
    cancel: &AtomicBool,
) -> io::Result<Journal> {
    let mut j = Journal::load(journal_path)?;
    progress.bytes_total.store(
        j.steps
            .iter()
            .filter(|s| s.state != StepState::Done)
            .map(|s| s.bytes)
            .sum(),
        Ordering::Relaxed,
    );
    progress.bytes_done.store(0, Ordering::Relaxed);
    let mut last_save = Instant::now();

    for i in 0..j.steps.len() {
        progress.step.store(i as u64, Ordering::Relaxed);
        if matches!(j.steps[i].state, StepState::Done | StepState::Failed) {
            continue;
        }
        let source = PathBuf::from(&j.steps[i].source);
        let target = PathBuf::from(&j.steps[i].target);

        if j.steps[i].state == StepState::Pending {
            if target.exists() {
                fail(&mut j, i, "the target already exists");
                j.save(journal_path)?;
                continue;
            }
            match list_files(&source) {
                Ok((files, links)) => {
                    j.steps[i].files = Some(files);
                    j.steps[i].skipped_links = links;
                    j.steps[i].state = StepState::Copying;
                }
                Err(e) => {
                    fail(&mut j, i, &e.to_string());
                    j.save(journal_path)?;
                    continue;
                }
            }
            j.save(journal_path)?;
        }

        if j.steps[i].state == StepState::Copying {
            let count = j.steps[i].files.as_ref().map_or(0, Vec::len);
            for f in 0..count {
                if cancel.load(Ordering::Relaxed) {
                    j.save(journal_path)?;
                    return Ok(j);
                }
                let file = j.steps[i].files.as_ref().expect("listed")[f].clone();
                if file.verified {
                    progress.bytes_done.fetch_add(file.bytes, Ordering::Relaxed);
                    continue;
                }
                if let Err(e) = copy_verified(&join(&source, &file.rel), &join(&target, &file.rel))
                {
                    fail(&mut j, i, &e.to_string());
                    break;
                }
                j.steps[i].files.as_mut().expect("listed")[f].verified = true;
                progress.bytes_done.fetch_add(file.bytes, Ordering::Relaxed);
                if last_save.elapsed() > Duration::from_secs(2) {
                    j.save(journal_path)?;
                    last_save = Instant::now();
                }
            }
            if j.steps[i].state == StepState::Copying {
                j.steps[i].state = StepState::Verified;
            }
            j.save(journal_path)?;
        }

        if j.steps[i].state == StepState::Verified {
            // Point the tool to the new place before the old one goes away.
            if let Some(var) = j.steps[i].env_var.clone()
                && j.steps[i].env_change.is_none()
            {
                let change = EnvChange {
                    name: var.clone(),
                    old: fx.get_env(&var),
                    new: target.to_string_lossy().into_owned(),
                };
                if let Err(e) = fx.set_env(&var, Some(&change.new)) {
                    fail(
                        &mut j,
                        i,
                        &format!("copied, but {var} could not be set: {e}"),
                    );
                    j.save(journal_path)?;
                    continue;
                }
                j.steps[i].env_change = Some(change);
                j.save(journal_path)?;
            }
            match fx.recycle(&source) {
                Ok(()) => j.steps[i].state = StepState::Done,
                Err(e) => fail(
                    &mut j,
                    i,
                    &format!(
                        "copied and verified, but the original could not go to the Recycle Bin: {e}"
                    ),
                ),
            }
            j.save(journal_path)?;
        }
    }
    j.finished = j
        .steps
        .iter()
        .all(|s| matches!(s.state, StepState::Done | StepState::Failed));
    j.save(journal_path)?;
    Ok(j)
}

fn fail(j: &mut Journal, i: usize, why: &str) {
    j.steps[i].state = StepState::Failed;
    j.steps[i].error = Some(why.to_string());
}

/// Unfinished journals in `dir` (a crash or a cancel left them), newest first.
pub fn unfinished(dir: &Path) -> Vec<(PathBuf, Journal)> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut v: Vec<(PathBuf, Journal)> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .filter_map(|p| Journal::load(&p).ok().map(|j| (p, j)))
        .filter(|(_, j)| !j.finished)
        .collect();
    v.sort_by_key(|(_, j)| std::cmp::Reverse(j.created_ms));
    v
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::collections::HashMap;

    /// Test effects: "recycling" moves into a bin folder; environment in a map.
    struct FakeFx {
        bin: PathBuf,
        env: RefCell<HashMap<String, String>>,
    }
    impl Effects for FakeFx {
        fn recycle(&self, path: &Path) -> io::Result<()> {
            fs::create_dir_all(&self.bin)?;
            fs::rename(path, self.bin.join(path.file_name().unwrap()))
        }
        fn get_env(&self, name: &str) -> Option<String> {
            self.env.borrow().get(name).cloned()
        }
        fn set_env(&self, name: &str, value: Option<&str>) -> io::Result<()> {
            match value {
                Some(v) => self.env.borrow_mut().insert(name.into(), v.into()),
                None => self.env.borrow_mut().remove(name),
            };
            Ok(())
        }
    }

    fn step(src: &Path, dst: &Path, env: Option<&str>) -> JournalStep {
        JournalStep {
            finding_id: "f".into(),
            name: "n".into(),
            source: src.to_string_lossy().into_owned(),
            target: dst.to_string_lossy().into_owned(),
            bytes: 0,
            env_var: env.map(str::to_string),
            env_change: None,
            state: StepState::Pending,
            error: None,
            files: None,
            skipped_links: 0,
        }
    }

    fn setup() -> (tempfile::TempDir, PathBuf, FakeFx) {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join(r"C\Users\me\Videos");
        fs::create_dir_all(src.join("sub")).unwrap();
        fs::write(src.join("a.mp4"), vec![1u8; 300_000]).unwrap();
        fs::write(src.join(r"sub\b.mkv"), vec![2u8; 1000]).unwrap();
        let fx = FakeFx {
            bin: dir.path().join("bin"),
            env: RefCell::new(HashMap::new()),
        };
        (dir, src, fx)
    }

    #[test]
    fn copies_verifies_then_recycles_and_sets_env() {
        let (dir, src, fx) = setup();
        let dst = dir.path().join(r"G\From C\Users\me\Videos");
        let jp = dir.path().join("j.json");
        Journal {
            id: 1,
            created_ms: 0,
            steps: vec![step(&src, &dst, Some("HF_HOME"))],
            finished: false,
        }
        .save(&jp)
        .unwrap();
        let p = MigrateProgress::default();
        let j = run(&jp, &fx, &p, &AtomicBool::new(false)).unwrap();
        assert!(j.finished);
        assert_eq!(j.steps[0].state, StepState::Done, "{:?}", j.steps[0].error);
        assert_eq!(fs::read(dst.join(r"sub\b.mkv")).unwrap(), vec![2u8; 1000]);
        assert!(!src.exists(), "the original went to the bin");
        assert!(fx.bin.join("Videos").join("a.mp4").exists());
        assert_eq!(
            fx.get_env("HF_HOME").as_deref(),
            Some(dst.to_string_lossy().as_ref())
        );
        assert_eq!(j.steps[0].env_change.as_ref().unwrap().old, None);
        assert_eq!(p.bytes_done.load(Ordering::Relaxed), 301_000);
    }

    #[test]
    fn resumes_after_a_cancel_or_crash() {
        let (dir, src, fx) = setup();
        let dst = dir.path().join("G").join("Videos");
        let jp = dir.path().join("j.json");
        Journal {
            id: 2,
            created_ms: 0,
            steps: vec![step(&src, &dst, None)],
            finished: false,
        }
        .save(&jp)
        .unwrap();
        // Cancelled before the first file: nothing copied, journal says Copying.
        let j = run(
            &jp,
            &fx,
            &MigrateProgress::default(),
            &AtomicBool::new(true),
        )
        .unwrap();
        assert_eq!(j.steps[0].state, StepState::Copying);
        assert!(!j.finished);
        assert_eq!(unfinished(dir.path()).len(), 1);
        // A half-written copy from a crash is overwritten on resume.
        fs::create_dir_all(&dst).unwrap();
        fs::write(dst.join("a.mp4.smdpart"), b"broken").unwrap();
        let j = run(
            &jp,
            &fx,
            &MigrateProgress::default(),
            &AtomicBool::new(false),
        )
        .unwrap();
        assert_eq!(j.steps[0].state, StepState::Done);
        assert!(!dst.join("a.mp4.smdpart").exists());
        assert_eq!(fs::read(dst.join("a.mp4")).unwrap().len(), 300_000);
        assert!(unfinished(dir.path()).is_empty());
    }

    #[test]
    fn refuses_an_existing_target_and_keeps_the_original() {
        let (dir, src, fx) = setup();
        let dst = dir.path().join("taken");
        fs::create_dir_all(&dst).unwrap();
        let jp = dir.path().join("j.json");
        Journal {
            id: 3,
            created_ms: 0,
            steps: vec![step(&src, &dst, None)],
            finished: false,
        }
        .save(&jp)
        .unwrap();
        let j = run(
            &jp,
            &fx,
            &MigrateProgress::default(),
            &AtomicBool::new(false),
        )
        .unwrap();
        assert_eq!(j.steps[0].state, StepState::Failed);
        assert!(src.join("a.mp4").exists());
    }

    #[cfg(windows)]
    #[test]
    fn skips_junctions_inside_the_source() {
        let (dir, src, fx) = setup();
        let outside = dir.path().join("outside");
        fs::create_dir_all(&outside).unwrap();
        fs::write(outside.join("secret.bin"), b"do not copy").unwrap();
        let link = src.join("link");
        let ok = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(&link)
            .arg(&outside)
            .output()
            .unwrap();
        assert!(ok.status.success());
        let dst = dir.path().join("dst");
        let jp = dir.path().join("j.json");
        Journal {
            id: 4,
            created_ms: 0,
            steps: vec![step(&src, &dst, None)],
            finished: false,
        }
        .save(&jp)
        .unwrap();
        let j = run(
            &jp,
            &fx,
            &MigrateProgress::default(),
            &AtomicBool::new(false),
        )
        .unwrap();
        assert_eq!(j.steps[0].skipped_links, 1);
        assert!(!dst.join(r"link\secret.bin").exists());
        assert!(
            outside.join("secret.bin").exists(),
            "the junction's target is untouched"
        );
    }
}
