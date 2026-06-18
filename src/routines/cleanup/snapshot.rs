//! Point-in-time `slug -> TTL` / `slug -> max-runtime` snapshots of the routine store, used to
//! drive a cleanup sweep without holding the store lock across filesystem and tmux work.

use std::collections::HashMap;

use super::super::command::slugify;
use super::super::model::RoutineStore;
use super::runtime::MAX_RUNTIME_SECS;
use super::ttl::MAX_TTL_SECS;

/// Snapshot each routine's `slug -> effective TTL` from the store.
///
/// Taken up front so the store lock is released before the sweep touches the filesystem and tmux —
/// reaping a directory tree must not block routine reads/writes.
pub fn snapshot_ttls(store: &RoutineStore) -> HashMap<String, u64> {
    let lock = store.lock().unwrap();
    lock.values()
        .map(|routine| (slugify(&routine.title), routine.effective_ttl_secs()))
        .collect()
}

/// Resolve a workbench slug's TTL against a [`snapshot_ttls`] map, falling back to
/// [`MAX_TTL_SECS`] for orphaned workbenches whose routine was since deleted.
pub fn ttl_for(snapshot: &HashMap<String, u64>, slug: &str) -> u64 {
    snapshot.get(slug).copied().unwrap_or(MAX_TTL_SECS)
}

/// Snapshot each routine's `slug -> effective max runtime` from the store. See [`snapshot_ttls`].
pub fn snapshot_max_runtimes(store: &RoutineStore) -> HashMap<String, u64> {
    let lock = store.lock().unwrap();
    lock.values()
        .map(|routine| {
            (
                slugify(&routine.title),
                routine.effective_max_runtime_secs(),
            )
        })
        .collect()
}

/// Resolve a workbench slug's max runtime against a [`snapshot_max_runtimes`] map, falling back to
/// [`MAX_RUNTIME_SECS`] for orphaned workbenches whose routine was since deleted.
pub fn max_runtime_for(snapshot: &HashMap<String, u64>, slug: &str) -> u64 {
    snapshot.get(slug).copied().unwrap_or(MAX_RUNTIME_SECS)
}

#[cfg(test)]
#[path = "snapshot_tests.rs"]
mod snapshot_tests;
