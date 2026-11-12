//! Where removed items go: permanent delete (opt-in), the disposable drive's quarantine
//! (default, when the drive is there and healthy), or the Recycle Bin.

use smd_core::health::DiskHealth;
use smd_core::organizer::Role;
use smd_win::cleanup::{DeleteMode, Destination, Item, Outcome, remove_items_to};

use crate::activity::UndoData;
use crate::state::{AppState, lock};
use crate::store::now_ms;

/// `S:\Quarantine` if quarantine is on and the disposable drive is present and healthy.
pub fn quarantine_root(state: &AppState) -> Option<std::path::PathBuf> {
    let settings = lock(&state.settings).ok()?.clone();
    if !settings.quarantine {
        return None;
    }
    let policy = crate::organizer::load_policy(state);
    let letter = policy.drive_for(Role::Disposable)?.to_string();
    if !smd_win::system::drives()
        .iter()
        .any(|d| d.letter.eq_ignore_ascii_case(&letter))
    {
        return None;
    }
    // Never write to a failing drive (Windows' health, SMART-based).
    let failing = smd_win::disks::physical_disks().ok()?.iter().any(|d| {
        d.letters.iter().any(|l| l.eq_ignore_ascii_case(&letter))
            && matches!(d.health, DiskHealth::Warning | DiskHealth::Unhealthy)
    });
    if failing {
        log::warn!("quarantine drive {letter} is not healthy: using the Recycle Bin");
        return None;
    }
    Some(smd_win::quarantine::root_for(&letter))
}

/// The result of a removal, with what an undo needs.
pub struct Removal {
    pub outcome: Outcome,
    pub undo: UndoData,
    /// The quarantine folder used, if any.
    pub quarantine: Option<String>,
    /// Something can be put back (Recycle Bin or quarantine, not permanent delete).
    pub undoable: bool,
}

/// Removes checked items to wherever the settings say. `id` is the activity id (also the batch id).
/// `allow_permanent` is false for your own files (My Clutter, app leftovers): they never skip the safety net.
pub fn remove(state: &AppState, id: u64, items: &[Item], allow_permanent: bool) -> Removal {
    let started = now_ms();
    let dest = match destination(state, id) {
        Destination::Permanent if !allow_permanent => match quarantine_root(state) {
            Some(root) => Destination::Quarantine {
                root,
                batch_id: id,
                now_ms: now_ms(),
            },
            None => Destination::RecycleBin,
        },
        d => d,
    };
    let (outcome, manifest) = remove_items_to(items, &dest);
    let quarantined: std::collections::HashSet<String> = manifest
        .as_deref()
        .and_then(|m| smd_win::quarantine::load(m).ok())
        .map(|b| b.items.into_iter().map(|i| i.original).collect())
        .unwrap_or_default();
    let undoable = !matches!(dest, Destination::Permanent) && !outcome.removed.is_empty();
    let undo = UndoData {
        started_unix_s: started / 1000,
        paths: outcome
            .removed
            .iter()
            .map(|r| r.0.clone())
            .filter(|p| !quarantined.contains(p))
            .collect(),
        quarantine: manifest
            .iter()
            .map(|m| m.to_string_lossy().into_owned())
            .collect(),
        ..Default::default()
    };
    let quarantine = match &dest {
        Destination::Quarantine { root, .. } if manifest.is_some() => {
            Some(root.to_string_lossy().into_owned())
        }
        _ => None,
    };
    Removal {
        outcome,
        undo,
        quarantine,
        undoable,
    }
}

pub fn destination(state: &AppState, batch_id: u64) -> Destination {
    let mode = lock(&state.settings)
        .map(|s| s.delete_mode)
        .unwrap_or_default();
    if mode == DeleteMode::Permanent {
        return Destination::Permanent;
    }
    match quarantine_root(state) {
        Some(root) => Destination::Quarantine {
            root,
            batch_id,
            now_ms: now_ms(),
        },
        None => Destination::RecycleBin,
    }
}
