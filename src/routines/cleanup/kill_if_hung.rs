
/// Watchdog decision for a single workbench: if its session is alive but the run has exceeded
/// `max_runtime_for(slug)` it is hung — `kill` the session, note it in the run's `agent.log`, and
/// report it as no longer alive. Returns whether the session should be treated as alive afterwards
/// (`true` only for a live session still within its bound).
///
/// Shared by [`reap_dir`] (full hourly sweep) and [`watchdog_dir()`] (short watchdog-only tick) so the
/// kill decision is defined once.
fn kill_if_hung(
    path: &Path,
    session: &str,
    ts: u64,
    now: u64,
    max_runtime: u64,
    is_alive: &dyn Fn(&str) -> bool,
    kill: &dyn Fn(&str),
) -> bool {
    if !is_alive(session) {
        return false;
    }
    if is_expired(now, ts, max_runtime) {
        // Hung run: force-kill the session so its workbench can be reaped under the normal TTL rules.
        kill(session);
        note_forced_kill(path);
        log::warn!("cleanup: killed routine session {session:?} exceeding max runtime");
        return false;
    }
    true
}

/// Scan `dir` and, for each `{slug}-{ts}` workbench:
///
/// 1. **Watchdog** — if its session is still alive but the run has exceeded `max_runtime_for(slug)`,
///    it is hung: `kill` its session, note it in the run's `agent.log`, and treat it as finished.
/// 2. **Reap** — a finished run (session not alive, originally or after the kill) whose
///    `ttl_for(slug)` has elapsed is removed.
///
/// A live session within its max runtime is left untouched. The TTL reap decision is measured from
/// each run's *finish* time (`finished_at(path, trigger_ts)`), not its trigger time, so a run is
/// kept for the full window after it completes (#174); the watchdog still measures elapsed runtime
/// from the trigger. `finished_at` is evaluated *before* the watchdog can force-kill the session, so
/// a hung run's forced-kill note (which touches `agent.log`) never masquerades as a fresh finish.
/// Returns the count of directories removed and the total bytes freed (summed only over trees
/// actually removed). `ttl_for`, `max_runtime_for`, `is_alive`, `kill`, `finished_at`, and `persist`
/// are injected so the decision logic is unit-testable without a filesystem clock or a live tmux
/// server. `persist` is called with `(slug, workbench name, workbench path, trigger ts, finish ts)`
/// right before removal, so a durable history record can be captured while the workbench (and its
/// `exit_code` file) still exists — see [`super::run_history`].
#[allow(
    clippy::too_many_arguments,
    reason = "each parameter is an independently injected test seam with no natural grouping"
)]
#[cfg(test)]
#[path = "cleanup_tests.rs"]
mod cleanup_tests;

#[cfg(test)]
#[path = "cleanup_tmux_tests.rs"]
mod cleanup_tmux_tests;

#[cfg(test)]
#[path = "cleanup_watchdog_tests.rs"]
mod cleanup_watchdog_tests;

#[cfg(test)]
#[path = "cleanup_claude_json_tests.rs"]
mod cleanup_claude_json_tests;

#[cfg(test)]
#[path = "cleanup_freed_bytes_tests.rs"]
mod cleanup_freed_bytes_tests;

#[cfg(test)]
#[path = "cleanup_run_history_tests.rs"]
mod cleanup_run_history_tests;
