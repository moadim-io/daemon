//! Auto-cleanup of finished routine runs.
//!
//! Triggering a routine creates a workbench at `~/.moadim/workbenches/{slug}-{ts}` and launches the
//! agent in a tmux session named `moadim-{slug}-{ts}`. When the agent exits the session ends, but
//! the workbench (prompt, logs, cloned repos) lingers forever. This module reaps those leftovers: a
//! workbench is removed once its run has *finished* (no live tmux session) **and** it is older than
//! the owning routine's [`Routine::effective_ttl_secs`]. Still-running sessions are never touched,
//! and orphaned workbenches (routine since deleted) fall back to [`DEFAULT_TTL_SECS`].

use std::path::Path;
use std::time::Duration;

use crate::paths::workbenches_dir;
use crate::utils::time::now_secs;

use super::command::slugify;
use super::model::RoutineStore;

mod ttl;
pub use ttl::DEFAULT_TTL_SECS;

/// How often the background task scans for expired workbenches.
pub const CLEANUP_INTERVAL: Duration = Duration::from_secs(60 * 60);

/// Split a workbench directory name into its `(slug, trigger_timestamp)`.
///
/// Names are `{slug}-{unix_secs}`; the timestamp is the trailing all-digit segment after the final
/// `-`. Returns `None` when the name has no such suffix or an empty slug (so unrelated directories
/// are skipped rather than reaped).
fn parse_workbench_name(name: &str) -> Option<(&str, u64)> {
    let (slug, ts) = name.rsplit_once('-')?;
    if slug.is_empty() || ts.is_empty() || !ts.bytes().all(|b| b.is_ascii_digit()) {
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
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Scan `dir`, removing each `{slug}-{ts}` workbench that is expired (`is_alive` returns `false` for
/// its session and `ttl_for(slug)` has elapsed). Returns the number of directories removed.
///
/// `ttl_for` and `is_alive` are injected so the decision logic is unit-testable without a filesystem
/// clock or a live tmux server.
fn reap_dir(
    dir: &Path,
    now: u64,
    ttl_for: &dyn Fn(&str) -> u64,
    is_alive: &dyn Fn(&str) -> bool,
) -> usize {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return 0;
    };
    let mut removed = 0;
    for entry in entries.flatten() {
        if !entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        let Some((slug, ts)) = parse_workbench_name(&name) else {
            continue;
        };
        if !is_expired(now, ts, ttl_for(slug)) {
            continue;
        }
        if is_alive(&format!("moadim-{name}")) {
            continue;
        }
        match std::fs::remove_dir_all(entry.path()) {
            Ok(()) => {
                removed += 1;
                log::info!("cleanup: removed expired workbench {name:?}");
            }
            Err(e) => log::warn!("cleanup: failed to remove workbench {name:?}: {e}"),
        }
    }
    removed
}

/// Remove finished, expired workbenches under `~/.moadim/workbenches/`, using each routine's TTL.
///
/// Returns the number of workbenches removed. Safe to call repeatedly; it only ever touches
/// directories whose run has ended.
pub fn cleanup_expired_workbenches(store: &RoutineStore) -> usize {
    // Snapshot slug -> ttl so the store lock is not held across filesystem and tmux calls.
    let ttls: std::collections::HashMap<String, u64> = {
        let lock = store.lock().unwrap();
        lock.values()
            .map(|r| (slugify(&r.title), r.effective_ttl_secs()))
            .collect()
    };
    let ttl_for = |slug: &str| ttls.get(slug).copied().unwrap_or(DEFAULT_TTL_SECS);
    reap_dir(
        &workbenches_dir(),
        now_secs(),
        &ttl_for,
        &tmux_session_alive,
    )
}

#[cfg(test)]
#[path = "cleanup_tests.rs"]
mod cleanup_tests;
