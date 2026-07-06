//! Size-based safety valve layered on top of the time-based TTL reaper (see the cleanup module).
//!
//! [`super::reap_dir`] only removes a workbench once it is *old enough*; nothing bounds how much
//! disk `~/.moadim/workbenches/` may hold while runs are still within TTL (a handful of concurrent
//! large repo clones can exhaust the disk long before any TTL elapses). This module adds an optional
//! total ceiling: once the tree exceeds [`MAX_DISK_BYTES_ENV`], finished workbenches (never a live
//! session) are evicted oldest-finished-first until back under it, regardless of their individual
//! TTL. Unset or `0` preserves today's unbounded-by-size behavior.

use std::path::{Path, PathBuf};

use super::{dir_size, parse_workbench_name, prune_claude_json, ReapStats};

/// Env var naming the total-disk ceiling for `~/.moadim/workbenches/`, in bytes. Unset, empty, or
/// unparsable means unbounded (today's behavior) — this is purely an additional safety valve on top
/// of TTL reaping, not a replacement for it.
pub(super) const MAX_DISK_BYTES_ENV: &str = "MOADIM_MAX_WORKBENCH_DISK_BYTES";

/// The configured ceiling, or `0` (unbounded) if [`MAX_DISK_BYTES_ENV`] is unset/unparsable.
pub(super) fn max_disk_bytes() -> u64 {
    std::env::var(MAX_DISK_BYTES_ENV)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(0)
}

/// One finished workbench eligible for cap-forced eviction.
pub(super) struct EvictCandidate {
    /// Workbench directory name (`{slug}-{ts}`), for logging.
    pub name: String,
    /// Absolute path to the workbench directory.
    pub path: PathBuf,
    /// Size in bytes of the workbench tree, as measured by [`super::dir_size`].
    pub size: u64,
    /// Best-effort finish time (unix seconds), oldest evicted first.
    pub finish_ts: u64,
}

/// Given every finished workbench eligible for eviction and the tree's `total_bytes` (summed over
/// *all* workbenches, live sessions included, since a live session still occupies disk even though
/// it can never be evicted), pick the oldest-finished-first subset to remove so the tree drops back
/// under `cap_bytes`. Returns an empty vec when `cap_bytes` is `0` (unbounded) or the tree is already
/// at or under it.
///
/// Pure decision logic — no filesystem access — so it is unit-testable with injected sizes/totals,
/// mirroring the injected-closure design of [`super::reap_dir`].
pub(super) fn pick_for_eviction(
    mut candidates: Vec<EvictCandidate>,
    cap_bytes: u64,
    total_bytes: u64,
) -> Vec<EvictCandidate> {
    if cap_bytes == 0 || total_bytes <= cap_bytes {
        return Vec::new();
    }
    candidates.sort_by_key(|candidate| candidate.finish_ts);
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

/// Post-TTL-reap safety valve: if `cap_bytes` is nonzero (see [`max_disk_bytes`]) and the tree under
/// `dir` still exceeds it, evict finished workbenches oldest-finished-first until back under the
/// cap. A live session (per `is_alive`) is never touched, no matter its size or age — see
/// [`pick_for_eviction`] for the pure selection logic this wraps with real directory/tmux/removal IO.
/// Returns the count removed and bytes freed by this pass alone.
pub(super) fn enforce(
    dir: &Path,
    cap_bytes: u64,
    is_alive: &dyn Fn(&str) -> bool,
    finished_at: &dyn Fn(&Path, u64) -> u64,
) -> ReapStats {
    if cap_bytes == 0 {
        return ReapStats::default();
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return ReapStats::default();
    };
    let mut total_bytes = 0u64;
    let mut candidates = Vec::new();
    for entry in entries.flatten() {
        if !entry.file_type().is_ok_and(|ft| ft.is_dir()) {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        let Some((_slug, ts)) = parse_workbench_name(&name) else {
            continue;
        };
        let size = dir_size(&entry.path());
        total_bytes += size;
        let session = format!("moadim-{name}");
        if is_alive(&session) {
            // Still occupies disk, but can never be evicted.
            continue;
        }
        candidates.push(EvictCandidate {
            name,
            path: entry.path(),
            size,
            finish_ts: finished_at(&entry.path(), ts),
        });
    }
    let mut stats = ReapStats::default();
    for candidate in pick_for_eviction(candidates, cap_bytes, total_bytes) {
        match std::fs::remove_dir_all(&candidate.path) {
            Ok(()) => {
                stats.removed += 1;
                stats.freed_bytes += candidate.size;
                log::warn!(
                    "cleanup: evicted not-yet-expired workbench {:?} ({} bytes) — over the {} cap",
                    candidate.name,
                    candidate.size,
                    MAX_DISK_BYTES_ENV
                );
                prune_claude_json(&candidate.path, &candidate.name);
            }
            Err(err) => {
                log::warn!(
                    "cleanup: failed to evict workbench {:?}: {err}",
                    candidate.name
                );
            }
        }
    }
    stats
}

#[cfg(test)]
#[path = "disk_cap_tests.rs"]
mod disk_cap_tests;

#[cfg(test)]
#[path = "enforce_disk_cap_tests.rs"]
mod enforce_disk_cap_tests;
