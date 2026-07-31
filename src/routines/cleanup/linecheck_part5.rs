
/// Remove finished, expired workbenches under `~/.moadim/workbenches/`, using each routine's TTL.
///
/// Returns the count of workbenches removed and the total bytes freed. Safe to call repeatedly; it
/// only ever touches directories whose run has ended. Also enforces the optional total-disk safety
/// valve (see [`disk_cap::enforce`]) once the normal TTL reap above has run, and sweeps the
/// repository mirror cache under `{config_dir}/cache/` (see [`repo_cache_cap::sweep`], issue #1425).
pub fn cleanup_expired_workbenches(store: &RoutineStore) -> ReapStats {
    let ttls = snapshot::snapshot_ttls(store);
    let max_runtimes = snapshot::snapshot_max_runtimes(store);
    let routine_ids = snapshot::snapshot_routine_ids(store);
    let ttl_for = |slug: &str| snapshot::ttl_for(&ttls, slug);
    let max_runtime_for = |slug: &str| snapshot::max_runtime_for(&max_runtimes, slug);
    // A workbench whose slug matches no current routine (deleted since) is skipped: there is no
    // routine's `runs.log` to attribute it to, and it's about to be removed anyway.
    let persist =
        |slug: &str, name: &str, workbench_path: &Path, started_at: u64, finished_at: u64| {
            let Some(routine_id) = routine_ids.get(slug) else {
                return;
            };
            if has_persisted_run(routine_id, name) {
                // Already recorded on a prior sweep whose `remove_dir_all` then failed, leaving
                // this workbench to be re-expired and re-persisted on the next sweep. Skip it so
                // one real run doesn't accumulate duplicate `runs.log` entries.
                return;
            }
            let exit_code = read_exit_code(workbench_path);
            let status = match exit_code {
                Some(0) => RunStatus::Success,
                Some(_) => RunStatus::Failed,
                None => RunStatus::Unknown,
            };
            append_persisted_run(
                routine_id,
                &PersistedRun {
                    workbench: name.to_string(),
                    started_at,
                    finished_at,
                    status,
                    exit_code,
                },
            );
            // Feed the same, already-deduplicated outcome into the failure circuit-breaker (#521);
            // see `circuit_breaker`'s doc comment for why this hook point (not `svc_list_runs`) was
            // chosen.
            circuit_breaker::record_run_outcome(store, routine_id, status);
        };
    let ttl_stats = reap_dir(
        &workbenches_dir(),
        now_secs(),
        &ttl_for,
        &max_runtime_for,
        &tmux_session_alive,
        &tmux_kill_session,
        &agent_log_finish_time,
        &persist,
    );
    let cap_stats = disk_cap::enforce(
        &workbenches_dir(),
        disk_cap::max_disk_bytes(),
        &tmux_session_alive,
        &agent_log_finish_time,
    );
    // Repository mirror cache (issue #1425): both its safety valves live behind one call — see
    // [`repo_cache_cap::sweep`].
    let repo_cache_stats = repo_cache_cap::sweep(store);
    let stats = ReapStats {
        removed: ttl_stats.removed + cap_stats.removed + repo_cache_stats.removed,
        freed_bytes: ttl_stats.freed_bytes + cap_stats.freed_bytes + repo_cache_stats.freed_bytes,
    };
    // Record this sweep for `moadim_cleanup_removed_total`/`moadim_cleanup_freed_bytes_total`
    // (see `counters`) — both the periodic background task and the on-demand `svc_cleanup` route
    // call this one function, so recording here covers both triggers.
    counters::record_sweep(stats.removed as u64, stats.freed_bytes);
    stats
}

/// Force-kill hung run sessions under `~/.moadim/workbenches/` that have exceeded their routine's
/// max runtime, without TTL-reaping finished workbenches.
///
/// Driven on the short [`WATCHDOG_INTERVAL`] cadence (separate from the hourly
/// [`cleanup_expired_workbenches`] sweep) so a sub-hour `max_runtime_secs` is enforced near its
/// bound rather than only at the next hourly tick. The killed workbench is reaped later by the
/// normal TTL sweep. Returns the number of sessions killed.
pub fn kill_hung_sessions(store: &RoutineStore) -> usize {
    let max_runtimes = snapshot::snapshot_max_runtimes(store);
    let max_runtime_for = |slug: &str| snapshot::max_runtime_for(&max_runtimes, slug);
    watchdog_dir(
        &workbenches_dir(),
        now_secs(),
        &max_runtime_for,
        &tmux_session_alive,
        &tmux_kill_session,
    )
}
