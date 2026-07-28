//! Two safety valves for `{config_dir}/cache/`, the persistent local mirror cache
//! [`crate::routines::command_repositories::clone_repository_stmts`] backs each routine's
//! declared `repositories` with (issue #466): nothing ever removed a mirror once cloned, so the
//! cache grew unbounded as routines were deleted, edited to a new URL, or a URL was a one-off
//! typo (issue #1425).
//!
//! Mirrors this module's sibling [`super::disk_cap`] (the workbench-tree size cap): an *orphan*
//! pass ([`prune_orphaned`]) removes any mirror no currently-stored routine's `repositories`
//! references anymore, and an optional total-size ceiling ([`MAX_REPO_CACHE_DISK_BYTES_ENV`])
//! evicts the least-recently-fetched mirrors still in use once the tree exceeds it. Unset or `0`
//! preserves today's unbounded behavior, the same convention as `MOADIM_MAX_WORKBENCH_DISK_BYTES`.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use super::super::model::RoutineStore;
use super::{dir_size, ReapStats};

/// Env var naming the total-disk ceiling for `{config_dir}/cache/`, in bytes. Unset, empty, or
/// unparsable means unbounded (today's behavior) — an additional safety valve layered on top of
/// [`prune_orphaned`], not a replacement for it.
pub(super) const MAX_REPO_CACHE_DISK_BYTES_ENV: &str = "MOADIM_MAX_REPO_CACHE_DISK_BYTES";

/// The configured ceiling, or `0` (unbounded) if [`MAX_REPO_CACHE_DISK_BYTES_ENV`] is
/// unset/unparsable.
pub(super) fn max_repo_cache_bytes() -> u64 {
    std::env::var(MAX_REPO_CACHE_DISK_BYTES_ENV)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(0)
}

/// Best-effort last-fetch time of a mirror, in unix seconds: the mtime of its `FETCH_HEAD` (
/// written by both the initial `git clone --mirror` and every later `git fetch --prune`, see
/// `clone_repository_stmts`). Falls back to the mirror directory's own mtime when `FETCH_HEAD` is
/// missing or unreadable, and to `0` (oldest possible, evicted first) when neither mtime is
/// readable — mirroring `agent_log_finish_time`'s fallback-to-directory convention in
/// [`super::agent_log_finish_time`].
pub(super) fn mirror_last_fetch_time(path: &Path) -> u64 {
    std::fs::metadata(path.join("FETCH_HEAD"))
        .or_else(|_err| std::fs::metadata(path))
        .and_then(|meta| meta.modified())
        .ok()
        .and_then(|mtime| mtime.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |elapsed| elapsed.as_secs())
}

/// Remove every directory under `dir` (the `{config_dir}/cache/` tree) whose name is not present
/// in `referenced` — i.e. orphaned by a routine deletion or a `repositories` URL edit (issue
/// #1425). `referenced` is a point-in-time snapshot of every currently-stored routine's repo-cache
/// directory *names* (see [`super::snapshot::snapshot_repo_cache_names`]), taken before this scan,
/// so a mirror that legitimately still matches a routine is never touched.
pub(super) fn prune_orphaned(dir: &Path, referenced: &HashSet<String>) -> ReapStats {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return ReapStats::default();
    };
    let mut stats = ReapStats::default();
    for entry in entries.flatten() {
        if !entry.file_type().is_ok_and(|ft| ft.is_dir()) {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        if referenced.contains(&name) {
            continue;
        }
        let size = dir_size(&entry.path());
        match std::fs::remove_dir_all(entry.path()) {
            Ok(()) => {
                stats.removed += 1;
                stats.freed_bytes += size;
                log::info!(
                    "cleanup: removed orphaned repo mirror {name:?} ({size} bytes) — no routine references it"
                );
            }
            Err(err) => {
                log::warn!("cleanup: failed to remove orphaned repo mirror {name:?}: {err}");
            }
        }
    }
    stats
}

/// One repo mirror eligible for cap-forced eviction (mirrors [`super::disk_cap::EvictCandidate`]).
pub(super) struct EvictCandidate {
    /// Mirror directory name, for logging.
    pub name: String,
    /// Absolute path to the mirror directory.
    pub path: PathBuf,
    /// Size in bytes of the mirror tree, as measured by [`super::dir_size`].
    pub size: u64,
    /// Best-effort last-fetch time (unix seconds, see [`mirror_last_fetch_time`]),
    /// least-recently-fetched evicted first.
    pub last_fetch: u64,
}

/// Given every mirror still present after [`prune_orphaned`] and the tree's `total_bytes`, pick
/// the least-recently-fetched subset to remove so the tree drops back under `cap_bytes`. Returns
/// an empty vec when `cap_bytes` is `0` (unbounded) or the tree is already at or under it.
///
/// Pure decision logic — no filesystem access — so it is unit-testable with injected
/// sizes/timestamps, mirroring [`super::disk_cap::pick_for_eviction`].
pub(super) fn pick_for_eviction(
    mut candidates: Vec<EvictCandidate>,
    cap_bytes: u64,
    total_bytes: u64,
) -> Vec<EvictCandidate> {
    if cap_bytes == 0 || total_bytes <= cap_bytes {
        return Vec::new();
    }
    candidates.sort_by_key(|candidate| candidate.last_fetch);
    let mut remaining = total_bytes;
    let mut chosen = Vec::new();
    for candidate in candidates {
        if remaining <= cap_bytes {
            break;
        }
        remaining = remaining.saturating_sub(candidate.size);
        chosen.push(candidate);
    }
    chosen
}

/// Post-orphan-prune safety valve: if `cap_bytes` is nonzero (see [`max_repo_cache_bytes`]) and
/// the tree under `dir` still exceeds it, evict mirrors least-recently-fetched-first (per
/// `last_fetch_for`) until back under the cap. Every mirror considered here is still referenced
/// by some routine — orphans are already gone by the time this runs (see [`prune_orphaned`]) —
/// but may still be evicted; it is simply re-cloned in full on that routine's next fire. Returns
/// the count removed and bytes freed by this pass alone. `last_fetch_for` (production:
/// [`mirror_last_fetch_time`]) is injected so the decision logic is unit-testable without relying
/// on real filesystem mtimes, mirroring [`super::disk_cap::enforce`]'s injected `finished_at`.
pub(super) fn enforce(
    dir: &Path,
    cap_bytes: u64,
    last_fetch_for: &dyn Fn(&Path) -> u64,
) -> ReapStats {
    if cap_bytes == 0 {
        return ReapStats::default();
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return ReapStats::default();
    };
    let mut total_bytes = 0_u64;
    let mut candidates = Vec::new();
    for entry in entries.flatten() {
        if !entry.file_type().is_ok_and(|ft| ft.is_dir()) {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        let size = dir_size(&entry.path());
        total_bytes += size;
        candidates.push(EvictCandidate {
            name,
            path: entry.path(),
            size,
            last_fetch: last_fetch_for(&entry.path()),
        });
    }
    let mut stats = ReapStats::default();
    for candidate in pick_for_eviction(candidates, cap_bytes, total_bytes) {
        match std::fs::remove_dir_all(&candidate.path) {
            Ok(()) => {
                stats.removed += 1;
                stats.freed_bytes += candidate.size;
                log::warn!(
                    "cleanup: evicted repo mirror {:?} ({} bytes) — over the {} cap",
                    candidate.name,
                    candidate.size,
                    MAX_REPO_CACHE_DISK_BYTES_ENV
                );
            }
            Err(err) => {
                log::warn!(
                    "cleanup: failed to evict repo mirror {:?}: {err}",
                    candidate.name
                );
            }
        }
    }
    stats
}

/// Total size in bytes of the whole `{config_dir}/cache/` tree — the persistent repository mirror
/// cache never pruned before issue #1425. Backs the `moadim_repo_cache_bytes` metric
/// (`crate::routes::metrics`), the same way `super::workbenches_total_bytes` backs
/// `moadim_workbench_bytes`.
pub(crate) fn total_bytes() -> u64 {
    dir_size(&crate::paths::repo_cache_root_dir())
}

/// Run both of this module's safety valves as one step of the periodic cleanup sweep (see
/// [`super::cleanup_expired_workbenches`]): [`prune_orphaned`] first, then [`enforce`] the
/// optional size cap on whatever mirrors remain. Snapshots `store` up front (see
/// [`super::snapshot::snapshot_repo_cache_names`]) so the store lock is released before this
/// touches disk, matching the TTL/disk-cap snapshots the workbench sweep already takes.
pub(super) fn sweep(store: &RoutineStore) -> ReapStats {
    let dir = crate::paths::repo_cache_root_dir();
    let referenced = super::snapshot::snapshot_repo_cache_names(store);
    let orphan_stats = prune_orphaned(&dir, &referenced);
    let cap_stats = enforce(&dir, max_repo_cache_bytes(), &mirror_last_fetch_time);
    ReapStats {
        removed: orphan_stats.removed + cap_stats.removed,
        freed_bytes: orphan_stats.freed_bytes + cap_stats.freed_bytes,
    }
}

#[cfg(test)]
#[path = "repo_cache_cap_tests.rs"]
mod repo_cache_cap_tests;
