#![allow(
    clippy::wildcard_imports,
    clippy::too_many_arguments,
    clippy::needless_pass_by_value,
    dead_code,
    reason = "split-out module preserves existing code while keeping files under the linecheck limit"
)]
use super::*;

/// Scan `dir` and force-kill any session that has exceeded its max runtime, *without* TTL-reaping
/// finished workbenches. This is the watchdog-only pass driven on the short [`WATCHDOG_INTERVAL`]
/// cadence, so a sub-hour `max_runtime_secs` is enforced near its bound instead of waiting for the
/// hourly [`reap_dir`] sweep. Returns the number of sessions killed. The injected `max_runtime_for`,
/// `is_alive`, and `kill` keep the decision logic unit-testable without a clock or a live tmux.
///
/// Also caps each workbench's `agent.log` to [`log_cap::MAX_AGENT_LOG_BYTES`] on this same tick
/// (#268): the raw `tmux pipe-pane` capture is unbounded and append-only, so a long or chatty run
/// could otherwise grow its log without limit between TTL sweeps.
pub(crate) fn watchdog_dir(
    dir: &Path,
    now: u64,
    max_runtime_for: &dyn Fn(&str) -> u64,
    is_alive: &dyn Fn(&str) -> bool,
    kill: &dyn Fn(&str),
) -> usize {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return 0;
    };
    let killed = std::cell::Cell::new(0_usize);
    let counting_kill = |session: &str| {
        killed.set(killed.get() + 1);
        kill(session);
    };
    for entry in entries.flatten() {
        if !entry.file_type().is_ok_and(|ft| ft.is_dir()) {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        let Some((slug, ts)) = parse_workbench_name(&name) else {
            continue;
        };
        log_cap::cap_agent_log_or_warn(&entry.path().join("agent.log"));
        let session = format!("moadim-{name}");
        kill_if_hung(
            &entry.path(),
            &session,
            ts,
            now,
            max_runtime_for(slug),
            is_alive,
            &counting_kill,
        );
    }
    killed.get()
}

/// Best-effort prune of the `projects[<path>]` entry from `~/.claude.json` after the workbench
/// directory at `path` (named `name`) was reaped, so the shared Claude Code config the built-in
/// `claude` agent seeds on every run (see `crate::routines::agents::claude_code`) does not
/// accumulate one dead entry per run, forever. Failures are logged, not propagated — a stale
/// `~/.claude.json` entry never blocks the wider cleanup sweep.
pub(crate) fn prune_claude_json(path: &Path, name: &str) {
    match prune_project(path) {
        Ok(true) => log::info!("cleanup: pruned stale ~/.claude.json entry for {name:?}"),
        Ok(false) => {}
        Err(err) => {
            log::warn!("cleanup: failed to prune ~/.claude.json entry for {name:?}: {err}");
        }
    }
}

pub(crate) fn reap_dir(
    dir: &Path,
    now: u64,
    ttl_for: &dyn Fn(&str) -> u64,
    max_runtime_for: &dyn Fn(&str) -> u64,
    is_alive: &dyn Fn(&str) -> bool,
    kill: &dyn Fn(&str),
    finished_at: &dyn Fn(&Path, u64) -> u64,
    persist: &dyn Fn(&str, &str, &Path, u64, u64),
) -> ReapStats {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return ReapStats::default();
    };
    let mut stats = ReapStats::default();
    for entry in entries.flatten() {
        if !entry.file_type().is_ok_and(|ft| ft.is_dir()) {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        let Some((slug, ts)) = parse_workbench_name(&name) else {
            continue;
        };
        // Captured before `kill_if_hung` below: a forced kill appends a note to `agent.log`,
        // which would otherwise bump its mtime to "now" and make a just-killed hung run look
        // like it *just* finished, resetting its retention window instead of reaping it.
        let finish_ts = finished_at(&entry.path(), ts);
        let session = format!("moadim-{name}");
        let alive = kill_if_hung(
            &entry.path(),
            &session,
            ts,
            now,
            max_runtime_for(slug),
            is_alive,
            kill,
        );
        if alive {
            // Still running within its max runtime — never touched.
            continue;
        }
        if !is_expired(now, finish_ts, ttl_for(slug)) {
            // Finished (or just killed) but its retention window has not elapsed yet — measured
            // from when the run finished, so its own duration does not eat into retention.
            continue;
        }
        // Record the run's outcome durably before the workbench (and its `exit_code` file) is
        // removed, so `svc_list_runs`/`svc_list_all_runs` still know about it afterwards.
        persist(slug, &name, &entry.path(), ts, finish_ts);
        // Measure the tree before deletion so a successful removal can report the space it reclaimed.
        let size = dir_size(&entry.path());
        match std::fs::remove_dir_all(entry.path()) {
            Ok(()) => {
                stats.removed += 1;
                stats.freed_bytes += size;
                log::info!("cleanup: removed expired workbench {name:?} (freed {size} bytes)");
                prune_claude_json(&entry.path(), &name);
            }
            Err(err) => log::warn!("cleanup: failed to remove workbench {name:?}: {err}"),
        }
    }
    stats
}

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
