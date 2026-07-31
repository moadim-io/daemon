
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
