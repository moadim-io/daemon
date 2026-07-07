//! Durable, append-only run history that survives workbench TTL reaping.
//!
//! A routine's workbenches are reaped once their TTL elapses (the cleanup module's `reap_dir`),
//! deleting the `exit_code` file [`svc_list_runs`](super::svc_list_runs) reads to derive a run's
//! outcome. Right before that deletion, the reaper appends a compact [`PersistedRun`] record to the
//! routine's `runs.log` ([`crate::paths::routine_run_history_path`], keyed by the routine's stable
//! UUID rather than its slug) — one JSON object per line — so the run's outcome survives past that
//! point, even though its `agent.log` body does not.

use std::io::Write as _;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::paths::routine_run_history_path;
use crate::routines::model::RunStatus;

/// One durable run record, appended to a routine's `runs.log` right before its workbench is
/// reaped. Unlike [`super::model::RunSummary`], `status` here is never [`RunStatus::Running`] — a
/// run is only ever persisted once its workbench is confirmed finished.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct PersistedRun {
    /// Workbench directory name (`{slug}-{unix_secs}`) the run ran under, before it was removed.
    pub workbench: String,
    /// Unix seconds the run was triggered.
    pub started_at: u64,
    /// Unix seconds the run finished.
    pub finished_at: u64,
    /// Success/failure/unknown; never `Running` (see the type-level doc).
    pub status: RunStatus,
    /// Process exit code, when recorded.
    pub exit_code: Option<i32>,
}

/// Parse the `exit_code` file under `workbench_path` (written by the launch command; see
/// `command::build_routine_command`), if present and parseable.
pub(crate) fn read_exit_code(workbench_path: &Path) -> Option<i32> {
    std::fs::read_to_string(workbench_path.join("exit_code"))
        .ok()
        .and_then(|text| text.trim().parse::<i32>().ok())
}

/// Size at which a routine's `runs.log` is rotated to a sibling `.log.1` file (replacing any
/// previous one) on the next append. The reaper appends one record here per finished run for the
/// whole life of the daemon with no other trim point, so a long-lived, frequently-firing routine's
/// history would otherwise grow unbounded — the same shape already fixed for `daemon.log` (see
/// [`crate::cli_system::DAEMON_LOG_MAX_BYTES`], #316). A routine's `runs.log` is far smaller per
/// entry and per-routine, so it gets a smaller cap.
const RUN_HISTORY_MAX_BYTES: u64 = 1024 * 1024;

/// Rotate `path` to a sibling `.1` file (overwriting any previous one) if it has grown past
/// [`RUN_HISTORY_MAX_BYTES`]. Best-effort: a failed rotation (permissions, race) falls through to
/// the caller's own `append(true)` open rather than blocking the reap sweep that triggered it.
fn rotate_run_history_if_oversized(path: &Path) {
    let Ok(metadata) = std::fs::metadata(path) else {
        return;
    };
    if metadata.len() <= RUN_HISTORY_MAX_BYTES {
        return;
    }
    let rotated_path = path.with_extension("log.1");
    let _ = std::fs::rename(path, rotated_path);
}

/// Append `run` as one NDJSON line to routine `id`'s `runs.log`.
///
/// Rotates the log first if it's grown past [`RUN_HISTORY_MAX_BYTES`] (see
/// [`rotate_run_history_if_oversized`]), then the append itself is best-effort: a write failure
/// (creating the routine's directory, opening the log, or the write itself — collapsed into a
/// single chain so there is one failure path to reason about, not three) is logged and swallowed
/// rather than blocking the reap sweep that triggered it — losing one history entry is far
/// cheaper than a stuck cleanup loop.
pub(crate) fn append_persisted_run(id: &str, run: &PersistedRun) {
    let line = serde_json::to_string(run).expect("PersistedRun always serializes");
    let path = routine_run_history_path(id);
    rotate_run_history_if_oversized(&path);
    let parent = path
        .parent()
        .expect("routine run-history path has a parent dir");
    let result = crate::utils::fs_perms::create_private_dir_all(parent)
        .and_then(|()| open_history_append(&path).and_then(|mut file| writeln!(file, "{line}")));
    if let Err(err) = result {
        log::warn!("run history: failed to append for routine {id:?}: {err}");
    }
}

/// Open `path` for append, creating it owner-only (`0600`) on unix if it doesn't already exist —
/// `runs.log` entries can echo `exit_code`s from an agent run and should stay unreadable by other
/// local accounts. Falls back to the standard `OpenOptions` on non-unix.
fn open_history_append(path: &Path) -> std::io::Result<std::fs::File> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .mode(0o600)
            .open(path)
    }
    #[cfg(not(unix))]
    {
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
    }
}

/// Read every persisted run for routine `id`. Order is not guaranteed — callers merge and sort
/// alongside live workbench-derived runs.
///
/// Malformed lines are skipped rather than failing the whole read: a single corrupted append (e.g.
/// from a crash mid-write) must not hide every run before or after it.
pub(crate) fn read_persisted_runs(id: &str) -> Vec<PersistedRun> {
    let Ok(text) = std::fs::read_to_string(routine_run_history_path(id)) else {
        return Vec::new();
    };
    text.lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect()
}

/// Whether routine `id` already has a persisted record for `workbench`.
///
/// A reap sweep persists a workbench's outcome *before* removing its directory (see
/// `cleanup::reap_dir`); if that removal then fails (permission hiccup, a still-open file, a crash),
/// the workbench survives and is expired again on the next sweep. Callers use this to skip
/// re-persisting a workbench that already made it into `runs.log`, so a stuck removal doesn't grow
/// an unbounded run of duplicate history entries for the same run.
pub(crate) fn has_persisted_run(id: &str, workbench: &str) -> bool {
    read_persisted_runs(id)
        .iter()
        .any(|run| run.workbench == workbench)
}

#[cfg(test)]
#[path = "run_history_tests.rs"]
mod run_history_tests;
