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
//! `~/.claude.json` (see [`crate::utils::claude_json`]), which the built-in `claude` agent's `setup`
//! step seeds on every run — otherwise that file would accumulate one dead entry per reaped run,
//! forever.

use std::path::Path;
use std::time::Duration;

use crate::paths::workbenches_dir;
use crate::utils::claude_json::prune_project;
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

/// How often the lightweight watchdog scans for *hung* runs to force-kill.
///
/// Decoupled from [`CLEANUP_INTERVAL`]: TTL-reaping finished workbenches can stay hourly, but the
/// max-runtime watchdog must fire on a much shorter cadence or a sub-hour `max_runtime_secs` is
/// unenforceable (a hung run would survive up to ~1h past its bound). At 30s the kill latency is
/// `effective_max_runtime_secs + <=30s`, so even a routine bounded to a few minutes is reaped near
/// its limit. This tick only evaluates the kill branch (no directory removal), so it stays cheap.
pub const WATCHDOG_INTERVAL: Duration = Duration::from_secs(30);

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

/// Scan `dir` and force-kill any session that has exceeded its max runtime, *without* TTL-reaping
/// finished workbenches. This is the watchdog-only pass driven on the short [`WATCHDOG_INTERVAL`]
/// cadence, so a sub-hour `max_runtime_secs` is enforced near its bound instead of waiting for the
/// hourly [`reap_dir`] sweep. Returns the number of sessions killed. The injected `max_runtime_for`,
/// `is_alive`, and `kill` keep the decision logic unit-testable without a clock or a live tmux.
fn watchdog_dir(
    dir: &Path,
    now: u64,
    max_runtime_for: &dyn Fn(&str) -> u64,
    is_alive: &dyn Fn(&str) -> bool,
    kill: &dyn Fn(&str),
) -> usize {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return 0;
    };
    let killed = std::cell::Cell::new(0usize);
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
/// `claude` agent seeds on every run (see [`crate::routines::agents::claude_code`]) does not
/// accumulate one dead entry per run, forever. Failures are logged, not propagated — a stale
/// `~/.claude.json` entry never blocks the wider cleanup sweep.
fn prune_claude_json(path: &Path, name: &str) {
    match prune_project(path) {
        Ok(true) => log::info!("cleanup: pruned stale ~/.claude.json entry for {name:?}"),
        Ok(false) => {}
        Err(err) => {
            log::warn!("cleanup: failed to prune ~/.claude.json entry for {name:?}: {err}");
        }
    }
}

/// Scan `dir` and, for each `{slug}-{ts}` workbench:
///
/// 1. **Watchdog** — if its session is still alive but the run has exceeded `max_runtime_for(slug)`,
///    it is hung: `kill` its session, note it in the run's `agent.log`, and treat it as finished.
/// 2. **Reap** — a finished run (session not alive, originally or after the kill) whose
///    `ttl_for(slug)` has elapsed is removed.
///
/// A live session within its max runtime is left untouched. Returns the number of directories
/// removed. `ttl_for`, `max_runtime_for`, `is_alive`, and `kill` are injected so the decision logic
/// is unit-testable without a filesystem clock or a live tmux server.
fn reap_dir(
    dir: &Path,
    now: u64,
    ttl_for: &dyn Fn(&str) -> u64,
    max_runtime_for: &dyn Fn(&str) -> u64,
    is_alive: &dyn Fn(&str) -> bool,
    kill: &dyn Fn(&str),
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
        if !is_expired(now, ts, ttl_for(slug)) {
            // Finished (or just killed) but its retention window has not elapsed yet.
            continue;
        }
        match std::fs::remove_dir_all(entry.path()) {
            Ok(()) => {
                removed += 1;
                log::info!("cleanup: removed expired workbench {name:?}");
                prune_claude_json(&entry.path(), &name);
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
    )
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

#[cfg(test)]
#[path = "cleanup_tests.rs"]
mod cleanup_tests;
