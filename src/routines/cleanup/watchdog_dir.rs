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
include!("cleanup_expired_workbenches.rs");
