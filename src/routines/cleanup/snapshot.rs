//! Point-in-time `slug -> TTL` / `slug -> max-runtime` snapshots of the routine store, used to
//! drive a cleanup sweep without holding the store lock across filesystem and tmux work.

use crate::utils::lock::LockRecover;
use std::collections::{HashMap, HashSet};

use super::super::command::slugify;
use super::super::model::RoutineStore;
use super::runtime::MAX_RUNTIME_SECS;
use super::ttl::MAX_TTL_SECS;

/// Snapshot each routine's `slug -> effective TTL` from the store.
///
/// Taken up front so the store lock is released before the sweep touches the filesystem and tmux —
/// reaping a directory tree must not block routine reads/writes.
pub fn snapshot_ttls(store: &RoutineStore) -> HashMap<String, u64> {
    let lock = store.lock_recover();
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
    let lock = store.lock_recover();
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

/// Snapshot each routine's `slug -> stable UUID` from the store, so a reaped workbench's outcome
/// can be persisted against its owning routine's durable `runs.log` (keyed by UUID, not slug — see
/// [`super::super::run_history`]) even though the reap sweep only has the workbench's slug to go on.
pub fn snapshot_routine_ids(store: &RoutineStore) -> HashMap<String, String> {
    let lock = store.lock_recover();
    lock.values()
        .map(|routine| (slugify(&routine.title), routine.id.clone()))
        .collect()
}

/// Snapshot the set of repo-cache directory *names* (see [`crate::paths::repo_cache_dir`]) every
/// currently-stored routine's `repositories` still references.
///
/// Backs the orphaned-mirror sweep ([`super::repo_cache_cap::prune_orphaned`], issue #1425): a
/// mirror whose name is absent from this snapshot was cloned for a repository no routine declares
/// anymore (the routine was deleted, or edited to a different URL) and is safe to remove. Taken up
/// front for the same reason as [`snapshot_ttls`] — the store lock must not be held across the
/// sweep's filesystem work.
pub fn snapshot_repo_cache_names(store: &RoutineStore) -> HashSet<String> {
    let lock = store.lock_recover();
    lock.values()
        .flat_map(|routine| &routine.repositories)
        .filter_map(|repo| {
            crate::paths::repo_cache_dir(&repo.repository)
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
        })
        .collect()
}

#[cfg(test)]
#[path = "snapshot_tests.rs"]
mod snapshot_tests;
