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

/// Append `run` as one NDJSON line to routine `id`'s `runs.log`.
///
/// Best-effort: a write failure (creating the routine's directory, opening the log, or the write
/// itself — collapsed into a single chain so there is one failure path to reason about, not three)
/// is logged and swallowed rather than blocking the reap sweep that triggered it — losing one
/// history entry is far cheaper than a stuck cleanup loop.
pub(crate) fn append_persisted_run(id: &str, run: &PersistedRun) {
    let line = serde_json::to_string(run).expect("PersistedRun always serializes");
    let path = routine_run_history_path(id);
    let parent = path
        .parent()
        .expect("routine run-history path has a parent dir");
    let result = std::fs::create_dir_all(parent).and_then(|()| {
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .and_then(|mut file| writeln!(file, "{line}"))
    });
    if let Err(err) = result {
        log::warn!("run history: failed to append for routine {id:?}: {err}");
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

#[cfg(test)]
#[path = "run_history_tests.rs"]
mod run_history_tests;
