//! Auto-cleanup of finished routine runs.
//!
//! Triggering a routine creates a workbench at `~/.moadim/workbenches/{slug}-{ts}` and launches the
//! agent in a tmux session named `moadim-{slug}-{ts}`. When the agent exits the session ends, but
//! the workbench (prompt, logs, cloned repos) lingers forever. This module reaps those leftovers: a
//! workbench is removed once its run has *finished* (no live tmux session) **and** it is older than
//! the owning routine's [`Routine::effective_ttl_secs`](crate::routines::Routine::effective_ttl_secs).
//! A still-running session within its
//! [`Routine::effective_max_runtime_secs`](crate::routines::Routine::effective_max_runtime_secs)
//! is never touched; one that has *exceeded* that bound is
//! a hung run, so a watchdog force-kills its tmux session (recording the reason in the run's
//! `agent.log`), after which the workbench is reaped under the normal TTL rules. Orphaned
//! workbenches (routine since deleted) fall back to `MAX_TTL_SECS` / `MAX_RUNTIME_SECS`.
//!
//! Reaping a workbench also prunes its matching `projects[<workbench>]` entry from the shared
//! `~/.claude.json` (see `crate::utils::claude_json`), which the built-in `claude` agent's `setup`
//! step seeds on every run — otherwise that file would accumulate one dead entry per reaped run,
//! forever.

use std::path::Path;
use std::time::Duration;

use crate::paths::workbenches_dir;
use crate::utils::claude_json::prune_project;
use crate::utils::time::now_secs;

use super::model::{RoutineStore, RunStatus};
use super::run_history::{append_persisted_run, has_persisted_run, read_exit_code, PersistedRun};

mod circuit_breaker;
mod counters;
mod disk_cap;
mod log_cap;
mod repo_cache_cap;
mod runtime;
mod session;
mod snapshot;
mod ttl;

use session::{note_forced_kill, tmux_kill_session, tmux_session_alive};

#[allow(
    clippy::missing_docs_in_private_items,
    reason = "split-out module keeps the file under the linecheck limit"
)]
mod linecheck_part2;
pub(crate) use linecheck_part2::*;
#[allow(
    clippy::missing_docs_in_private_items,
    reason = "split-out module keeps the file under the linecheck limit"
)]
mod linecheck_part3;
pub(crate) use linecheck_part3::*;

pub(crate) use counters::totals as cleanup_sweep_totals;
pub(crate) use repo_cache_cap::total_bytes as repo_cache_total_bytes;
pub(crate) use runtime::max_runtime_ceiling_secs;
pub(crate) use session::tmux_session_alive as run_session_alive;
pub(crate) use session::tmux_session_count;
pub(crate) use session::tmux_session_prefix_alive;
pub(crate) use ttl::ttl_ceiling_secs;

/// Total size in bytes of the whole `~/.moadim/workbenches/` tree, live and reaped-but-not-yet
/// -swept alike. Backs the `moadim_workbench_bytes` metric (`crate::routes::metrics`); a thin
/// wrapper so that route doesn't need to know this module's private [`dir_size`] walker exists.
pub(crate) fn workbenches_total_bytes() -> u64 {
    dir_size(&workbenches_dir())
}

/// How often the background task scans for expired workbenches.
///
/// A routine's `effective_ttl_secs` can be as low as the cron interval (e.g. ~60s for an
/// every-minute schedule, see [`ttl::MAX_TTL_SECS`]), well under an hour. This was previously a
/// flat 1h, so a high-frequency routine's finished workbenches (full repo clones included) could
/// pile up dozens deep between sweeps (#170). 5 minutes bounds that worst case to a handful of
/// stale workbenches while keeping the sweep infrequent enough that its directory walk and
/// `dir_size`/`remove_dir_all` work stay cheap.
pub const CLEANUP_INTERVAL: Duration = Duration::from_secs(5 * 60);

/// How often the lightweight watchdog scans for *hung* runs to force-kill.
///
/// Shorter than [`CLEANUP_INTERVAL`]: the max-runtime watchdog must fire on a cadence tight enough
/// that a sub-minute `max_runtime_secs` is still enforceable near its bound. At 30s the kill
/// latency is `effective_max_runtime_secs + <=30s`. This tick only evaluates the kill branch (no
/// directory removal), so it stays cheap.
pub const WATCHDOG_INTERVAL: Duration = Duration::from_secs(30);

/// Split a workbench directory name into its `(slug, trigger_timestamp)`.
///
/// Names are `{slug}-{unix_secs}` or, since #411, `{slug}-{unix_secs}_{pid}` — a PID suffix joined
/// with `_` makes the run id collision-resistant for two same-second runs of one routine. The
/// timestamp is the all-digit `{unix_secs}` segment after the final `-` (with any trailing `_{pid}`
/// stripped). Slugs are `[a-z0-9-]` only, so the `_` boundary is unambiguous and legacy
/// `{slug}-{unix_secs}` names keep parsing. Returns `None` when the name has no such suffix or an
/// empty slug (so unrelated directories are skipped rather than reaped).
pub(super) fn parse_workbench_name(name: &str) -> Option<(&str, u64)> {
    let (slug, rest) = name.rsplit_once('-')?;
    // Drop the optional `_{pid}` run-id suffix; the leading segment is the trigger timestamp.
    let ts = rest.split_once('_').map_or(rest, |(secs, _pid)| secs);
    if slug.is_empty() || ts.is_empty() || !ts.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    Some((slug, ts.parse().ok()?))
}

/// Whether a workbench triggered at `ts` has outlived `ttl` as of `now` (saturating, so clock skew
/// that puts `ts` in the future reads as age 0, never expired).
const fn is_expired(now: u64, ts: u64, ttl: u64) -> bool {
    now.saturating_sub(ts) > ttl
}

/// Outcome of a cleanup sweep: how many workbenches were reaped and the disk space reclaimed.
///
/// `freed_bytes` is summed across each removed workbench's tree, measured just before deletion, so
/// operators (and `--json` consumers) learn the payoff of a sweep rather than a bare directory count
/// — a removed workbench can hold cloned repos worth tens or hundreds of MB.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ReapStats {
    /// Number of finished, expired run workbenches removed by this sweep.
    pub removed: usize,
    /// Total bytes freed, summed across the trees of the workbenches actually removed.
    pub freed_bytes: u64,
}

/// Total size in bytes of every file under `path`, walked recursively. Best-effort: unreadable
/// entries are skipped (yielding a lower bound rather than failing), and directory symlinks are not
/// traversed, so a workbench tree cannot send the walk into a cycle.
fn dir_size(path: &Path) -> u64 {
    let Ok(entries) = std::fs::read_dir(path) else {
        return 0;
    };
    let mut total = 0;
    for entry in entries.flatten() {
        // `file_type()` does not follow symlinks, so a symlinked directory reads as a non-dir and is
        // counted by its own (small) metadata length instead of being descended into.
        if entry.file_type().is_ok_and(|ft| ft.is_dir()) {
            total += dir_size(&entry.path());
        } else {
            total += entry.metadata().map_or(0, |meta| meta.len());
        }
    }
    total
}

/// Best-effort *finish* time of a finished run, as unix seconds: the mtime of its `agent.log` (the
/// last time the agent wrote output). Falls back to `trigger_ts` when the log is missing or its
/// mtime is unreadable, and is clamped to at least `trigger_ts` so retention is never measured from
/// a moment earlier than the run's own start.
///
/// Retention (TTL) is measured from finish, not from trigger (#174): a run consumes none of its
/// keep-window while still executing, so a long run — or any run on a short-interval schedule — is
/// still retained for the full `effective_ttl_secs` after it completes. The max-runtime watchdog
/// continues to measure from `trigger_ts` (elapsed wall-clock since launch), which is correct.
fn agent_log_finish_time(dir: &Path, trigger_ts: u64) -> u64 {
    std::fs::metadata(dir.join("agent.log"))
        .and_then(|meta| meta.modified())
        .ok()
        .and_then(|mtime| mtime.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(trigger_ts, |elapsed| elapsed.as_secs().max(trigger_ts))
}

/// Watchdog decision for a single workbench: if its session is alive but the run has exceeded
/// `max_runtime_for(slug)` it is hung — `kill` the session, note it in the run's `agent.log`, and
/// report it as no longer alive. Returns whether the session should be treated as alive afterwards
/// (`true` only for a live session still within its bound).
///
/// Shared by [`reap_dir`] (full hourly sweep) and [`watchdog_dir`] (short watchdog-only tick) so the
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
