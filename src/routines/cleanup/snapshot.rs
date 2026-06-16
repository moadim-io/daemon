//! Point-in-time `slug -> TTL` snapshot of the routine store, used to drive a cleanup sweep.

use std::collections::HashMap;

use super::super::command::slugify;
use super::super::model::RoutineStore;
use super::ttl::MAX_TTL_SECS;

/// Snapshot each routine's `slug -> effective TTL` from the store.
///
/// Taken up front so the store lock is released before the sweep touches the filesystem and tmux —
/// reaping a directory tree must not block routine reads/writes.
pub fn snapshot_ttls(store: &RoutineStore) -> HashMap<String, u64> {
    let lock = store.lock().unwrap();
    lock.values()
        .map(|r| (slugify(&r.title), r.effective_ttl_secs()))
        .collect()
}

/// Resolve a workbench slug's TTL against a [`snapshot_ttls`] map, falling back to
/// [`MAX_TTL_SECS`] for orphaned workbenches whose routine was since deleted.
pub fn ttl_for(snapshot: &HashMap<String, u64>, slug: &str) -> u64 {
    snapshot.get(slug).copied().unwrap_or(MAX_TTL_SECS)
}
