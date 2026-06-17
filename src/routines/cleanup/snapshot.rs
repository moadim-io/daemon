//! Point-in-time `slug -> run limits` snapshot of the routine store, used to drive a cleanup sweep.

use std::collections::HashMap;

use super::super::command::slugify;
use super::super::model::RoutineStore;
use super::ttl::{DEFAULT_MAX_RUNTIME_SECS, MAX_TTL_SECS};

/// A routine's cleanup-relevant limits, snapshotted from the store.
#[derive(Debug, Clone, Copy)]
pub struct RunLimits {
    /// Retention for finished workbenches (`Routine::effective_ttl_secs`).
    pub ttl_secs: u64,
    /// Max wall-clock runtime before the watchdog kills a live session
    /// (`Routine::effective_max_runtime_secs`).
    pub max_runtime_secs: u64,
}

/// Snapshot each routine's `slug -> RunLimits` from the store.
///
/// Taken up front so the store lock is released before the sweep touches the filesystem and tmux —
/// reaping a directory tree (or killing a session) must not block routine reads/writes.
pub fn snapshot_limits(store: &RoutineStore) -> HashMap<String, RunLimits> {
    let lock = store.lock().unwrap();
    lock.values()
        .map(|routine| {
            (
                slugify(&routine.title),
                RunLimits {
                    ttl_secs: routine.effective_ttl_secs(),
                    max_runtime_secs: routine.effective_max_runtime_secs(),
                },
            )
        })
        .collect()
}

/// Resolve a workbench slug's TTL against a [`snapshot_limits`] map, falling back to
/// [`MAX_TTL_SECS`] for orphaned workbenches whose routine was since deleted.
pub fn ttl_for(snapshot: &HashMap<String, RunLimits>, slug: &str) -> u64 {
    snapshot
        .get(slug)
        .map(|limits| limits.ttl_secs)
        .unwrap_or(MAX_TTL_SECS)
}

/// Resolve a workbench slug's max runtime against a [`snapshot_limits`] map, falling back to
/// [`DEFAULT_MAX_RUNTIME_SECS`] for orphaned workbenches so a hung orphan is still eventually killed.
pub fn max_runtime_for(snapshot: &HashMap<String, RunLimits>, slug: &str) -> u64 {
    snapshot
        .get(slug)
        .map(|limits| limits.max_runtime_secs)
        .unwrap_or(DEFAULT_MAX_RUNTIME_SECS)
}
