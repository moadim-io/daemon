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

use std::path::Path;
use std::time::Duration;

use crate::paths::workbenches_dir;
use crate::utils::time::now_secs;

use super::model::RoutineStore;

mod runtime;
mod session;
mod snapshot;
mod ttl;

use session::{note_forced_kill, tmux_kill_session, tmux_session_alive};

pub(crate) use runtime::max_runtime_ceiling_secs;
pub(crate) use ttl::ttl_ceiling_secs;

/// How often the background task scans for expired workbenches.
pub const CLEANUP_INTERVAL: Duration = Duration::from_secs(60 * 60);

/// Split a workbench directory name into its `(slug, trigger_timestamp)`.
///
/// Names are `{slug}-{unix_secs}`; the timestamp is the trailing all-digit segment after the final
/// `-`. Returns `None` when the name has no such suffix or an empty slug (so unrelated directories
/// are skipped rather than reaped).
pub(super) fn parse_workbench_name(name: &str) -> Option<(&str, u64)> {
    let (slug, ts) = name.rsplit_once('-')?;
    if slug.is_empty() || ts.is_empty() || !ts.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    Some((slug, ts.parse().ok()?))
}

/// Whether a workbench triggered at `ts` has outlived `ttl` as of `now` (saturating, so clock skew
/// that puts `ts` in the future reads as age 0, never expired).
fn is_expired(now: u64, ts: u64, ttl: u64) -> bool {
    now.saturating_sub(ts) > ttl
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
        .map(|elapsed| elapsed.as_secs().max(trigger_ts))
        .unwrap_or(trigger_ts)
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
/// from the trigger. Returns the number of directories removed. `ttl_for`, `max_runtime_for`,
/// `is_alive`, `kill`, and `finished_at` are injected so the decision logic is unit-testable
/// without a filesystem clock or a live tmux server.
fn reap_dir(
    dir: &Path,
    now: u64,
    ttl_for: &dyn Fn(&str) -> u64,
    max_runtime_for: &dyn Fn(&str) -> u64,
    is_alive: &dyn Fn(&str) -> bool,
    kill: &dyn Fn(&str),
    finished_at: &dyn Fn(&Path, u64) -> u64,
) -> usize {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return 0;
    };
    let mut removed = 0;
    for entry in entries.flatten() {
        if !entry.file_type().is_ok_and(|ft| ft.is_dir()) {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        let Some((slug, ts)) = parse_workbench_name(&name) else {
            continue;
        };
        let session = format!("moadim-{name}");
        let mut alive = is_alive(&session);
        if alive && is_expired(now, ts, max_runtime_for(slug)) {
            // Hung run: force-kill the session so its workbench can be reaped below.
            kill(&session);
            note_forced_kill(&entry.path());
            log::warn!("cleanup: killed routine session {session:?} exceeding max runtime");
            alive = false;
        }
        if alive {
            // Still running within its max runtime — never touched.
            continue;
        }
        let finish_ts = finished_at(&entry.path(), ts);
        if !is_expired(now, finish_ts, ttl_for(slug)) {
            // Finished (or just killed) but its retention window has not elapsed yet — measured
            // from when the run finished, so its own duration does not eat into retention.
            continue;
        }
        match std::fs::remove_dir_all(entry.path()) {
            Ok(()) => {
                removed += 1;
                log::info!("cleanup: removed expired workbench {name:?}");
            }
            Err(err) => log::warn!("cleanup: failed to remove workbench {name:?}: {err}"),
        }
    }
    removed
}

/// Remove finished, expired workbenches under `~/.moadim/workbenches/`, using each routine's TTL.
///
/// Returns the number of workbenches removed. Safe to call repeatedly; it only ever touches
/// directories whose run has ended.
pub fn cleanup_expired_workbenches(store: &RoutineStore) -> usize {
    let ttls = snapshot::snapshot_ttls(store);
    let max_runtimes = snapshot::snapshot_max_runtimes(store);
    let ttl_for = |slug: &str| snapshot::ttl_for(&ttls, slug);
    let max_runtime_for = |slug: &str| snapshot::max_runtime_for(&max_runtimes, slug);
    reap_dir(
        &workbenches_dir(),
        now_secs(),
        &ttl_for,
        &max_runtime_for,
        &tmux_session_alive,
        &tmux_kill_session,
        &agent_log_finish_time,
    )
}

#[cfg(test)]
#[path = "cleanup_tests.rs"]
mod cleanup_tests;
