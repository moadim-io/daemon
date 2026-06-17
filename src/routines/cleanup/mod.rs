//! Auto-cleanup of finished routine runs.
//!
//! Triggering a routine creates a workbench at `~/.moadim/workbenches/{slug}-{ts}` and launches the
//! agent in a tmux session named `moadim-{slug}-{ts}`. When the agent exits the session ends, but
//! the workbench (prompt, logs, cloned repos) lingers forever. This module reaps those leftovers: a
//! workbench is removed once its run has *finished* (no live tmux session) **and** it is older than
//! the owning routine's [`Routine::effective_ttl_secs`]. Still-running sessions are never touched,
//! and orphaned workbenches (routine since deleted) fall back to `MAX_TTL_SECS`.
//!
//! A second rule handles the one case the TTL reaper cannot: a run that never *finishes*. A session
//! whose run has outlived its routine's [`Routine::effective_max_runtime_secs`] is force-killed
//! (`tmux kill-session`) and its forced termination recorded in the run's `agent.log`; the workbench
//! is then eligible for normal TTL reaping. This bounds the blast radius of a hung agent and stops
//! stuck sessions accumulating one per cron tick.

use std::io::Write;
use std::path::Path;
use std::time::Duration;

use crate::paths::workbenches_dir;
use crate::utils::time::now_secs;

use super::model::RoutineStore;

mod snapshot;
mod ttl;

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

/// Return `true` if a tmux session named `session` currently exists.
///
/// Uses an exact (`=`) target match so `moadim-foo-1` never matches `moadim-foo-10`. A missing
/// `tmux` binary (exit status unavailable) is treated as "not alive": with no tmux there is no
/// running session to protect, so an expired workbench is safe to reap.
fn tmux_session_alive(session: &str) -> bool {
    std::process::Command::new("tmux")
        .arg("has-session")
        .arg("-t")
        .arg(format!("={session}"))
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

/// Force-kill the tmux session named `session`.
///
/// Uses an exact (`=`) target match so `moadim-foo-1` never matches `moadim-foo-10`. Best-effort: a
/// missing `tmux` binary or an already-gone session is ignored — the goal (no live session) is met
/// either way.
fn tmux_kill_session(session: &str) {
    let _ = std::process::Command::new("tmux")
        .arg("kill-session")
        .arg("-t")
        .arg(format!("={session}"))
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
}

/// Append a forced-termination note to a workbench's `agent.log`, so the reason a run ended is
/// visible alongside the agent's own output. Best-effort: a write failure is logged and ignored.
fn record_forced_termination(workbench: &Path, runtime_secs: u64, max_runtime_secs: u64) {
    let log_path = workbench.join("agent.log");
    let line = format!(
        "moadim: routine exceeded max runtime ({runtime_secs}s > {max_runtime_secs}s); killing session\n"
    );
    match std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
    {
        Ok(mut file) => {
            if let Err(err) = file.write_all(line.as_bytes()) {
                log::warn!("cleanup: failed to write {log_path:?}: {err}");
            }
        }
        Err(err) => log::warn!("cleanup: failed to open {log_path:?}: {err}"),
    }
}

/// Scan `dir`, applying the watchdog then the TTL reaper to each `{slug}-{ts}` workbench. Returns
/// the number of directories removed.
///
/// For each workbench:
/// 1. **Watchdog** — if its session is still alive but its run has outlived `max_runtime_for(slug)`,
///    `kill_session` it and append a note to the run's `agent.log`. A live session still *within*
///    its max runtime is left untouched and the workbench skipped (a running run is never reaped).
/// 2. **Reaper** — a finished run (no live session, or one just killed above) whose age exceeds
///    `ttl_for(slug)` is removed; otherwise it is kept until its TTL elapses on a later sweep.
///
/// `ttl_for`, `max_runtime_for`, `is_alive`, and `kill_session` are injected so the decision logic is
/// unit-testable without a filesystem clock or a live tmux server.
fn reap_dir(
    dir: &Path,
    now: u64,
    ttl_for: &dyn Fn(&str) -> u64,
    max_runtime_for: &dyn Fn(&str) -> u64,
    is_alive: &dyn Fn(&str) -> bool,
    kill_session: &dyn Fn(&str),
) -> usize {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return 0;
    };
    let mut removed = 0;
    for entry in entries.flatten() {
        if !entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false) {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        let Some((slug, ts)) = parse_workbench_name(&name) else {
            continue;
        };
        let session = format!("moadim-{name}");
        if is_alive(&session) {
            let runtime = now.saturating_sub(ts);
            let max_runtime = max_runtime_for(slug);
            if runtime <= max_runtime {
                // A running run within its max runtime is never touched.
                continue;
            }
            // Hung run: force-kill the session and record why, so it becomes reapable below.
            kill_session(&session);
            record_forced_termination(&entry.path(), runtime, max_runtime);
            log::warn!("cleanup: killed session {session:?} after {runtime}s (max {max_runtime}s)");
        }
        if !is_expired(now, ts, ttl_for(slug)) {
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

/// Remove finished, expired workbenches under `~/.moadim/workbenches/`, force-killing any session
/// that has exceeded its routine's max runtime first. Uses each routine's TTL and max runtime.
///
/// Returns the number of workbenches removed. Safe to call repeatedly; it only reaps directories
/// whose run has ended (or was just killed for overrunning).
pub fn cleanup_expired_workbenches(store: &RoutineStore) -> usize {
    let limits = snapshot::snapshot_limits(store);
    let ttl_for = |slug: &str| snapshot::ttl_for(&limits, slug);
    let max_runtime_for = |slug: &str| snapshot::max_runtime_for(&limits, slug);
    reap_dir(
        &workbenches_dir(),
        now_secs(),
        &ttl_for,
        &max_runtime_for,
        &tmux_session_alive,
        &tmux_kill_session,
    )
}

#[cfg(test)]
#[path = "cleanup_tests.rs"]
mod cleanup_tests;
