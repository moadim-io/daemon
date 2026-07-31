//! Two safety valves for `{config_dir}/cache/`, the persistent local mirror cache
//! [`crate::routines::command::clone_repository_stmts`] backs each routine's
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
include!("repo_cache_cap_part2.rs");
