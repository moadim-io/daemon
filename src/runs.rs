//! Per-run execution records for cron jobs: capture, JSONL persistence, and the JSON model
//! shared by the REST `GET /cron-jobs/{id}/runs` route and the `cron_job_runs` MCP tool.
//!
//! Every time a job's handler is spawned — today, only via a manual trigger (REST `/trigger`, the
//! MCP `trigger_cron_job` tool, or the UI's ▶ button) — a [`RunRecord`] capturing its exit code,
//! timing, and truncated stdout/stderr is appended to `~/.config/moadim/jobs/{id}/runs.jsonl`.
//!
//! Scheduled fires are not yet captured here: the OS crontab invokes a managed job's handler
//! directly (see [`crate::sync::format_crontab_line`]), bypassing this daemon process entirely, so
//! there is no in-process hook to observe them. The [`RunTrigger::Scheduled`] variant and the
//! storage/REST/MCP/UI surfaces are still fully wired so scheduled-run capture can land later
//! without further schema changes.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::process::Command;
use std::thread::{self, JoinHandle};
use uuid::Uuid;

use crate::cron_jobs::CronJob;
use crate::utils::time::now_secs;

/// Maximum bytes of captured stdout/stderr retained per run; longer output is truncated.
const MAX_OUTPUT_BYTES: usize = 10 * 1024;

/// Maximum number of run records retained per job; the oldest are dropped on overflow.
const MAX_RUNS_PER_JOB: usize = 100;

/// How a run was initiated.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, utoipa::ToSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum RunTrigger {
    /// Fired by the job's cron schedule.
    Scheduled,
    /// Fired by a manual trigger (REST `/trigger`, the MCP `trigger_cron_job` tool, or the UI).
    Manual,
}

/// A single recorded execution of a cron job's handler.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, utoipa::ToSchema)]
pub struct RunRecord {
    /// Unique identifier (UUID v4) for this run.
    pub id: String,
    /// ID of the cron job this run belongs to.
    pub job_id: String,
    /// Unix timestamp (seconds) when the run started.
    pub started_at: u64,
    /// Unix timestamp (seconds) when the run finished.
    pub finished_at: u64,
    /// Wall-clock duration of the run, in milliseconds.
    pub duration_ms: u64,
    /// Process exit code, or `None` if the handler could not be spawned.
    pub exit_code: Option<i32>,
    /// How the run was initiated.
    pub trigger: RunTrigger,
    /// Captured standard output, truncated to [`MAX_OUTPUT_BYTES`].
    pub stdout: String,
    /// Captured standard error, truncated to [`MAX_OUTPUT_BYTES`].
    pub stderr: String,
}

/// Truncate `text` to at most [`MAX_OUTPUT_BYTES`] bytes, respecting UTF-8 character boundaries so
/// the result is always valid `str` data.
fn truncate_output(text: &str) -> String {
    if text.len() <= MAX_OUTPUT_BYTES {
        return text.to_string();
    }
    let mut end = MAX_OUTPUT_BYTES;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    text[..end].to_string()
}

/// Execute the handler at `handler_path` synchronously, capturing stdout/stderr and timing, and
/// return the resulting [`RunRecord`] for `job`. Returns a record with `exit_code: None` (and no
/// captured output) when `handler_path` does not exist or the process could not be spawned —
/// mirroring the best-effort, log-and-continue behavior [`crate::utils::process::spawn_and_reap`]
/// uses elsewhere in the trigger path.
fn execute_and_capture(
    job: &CronJob,
    handler_path: &std::path::Path,
    trigger: RunTrigger,
) -> RunRecord {
    let started_at = now_secs();
    let start = std::time::Instant::now();
    let (exit_code, stdout, stderr) = if handler_path.exists() {
        match Command::new(handler_path).output() {
            Ok(output) => (
                output.status.code(),
                String::from_utf8_lossy(&output.stdout).into_owned(),
                String::from_utf8_lossy(&output.stderr).into_owned(),
            ),
            Err(err) => {
                log::warn!("trigger: failed to spawn handler {handler_path:?}: {err}");
                (None, String::new(), String::new())
            }
        }
    } else {
        log::warn!("trigger: handler script not found at {handler_path:?}");
        (None, String::new(), String::new())
    };
    let duration_ms = u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX);
    RunRecord {
        id: Uuid::new_v4().to_string(),
        job_id: job.id.clone(),
        started_at,
        finished_at: now_secs(),
        duration_ms,
        exit_code,
        trigger,
        stdout: truncate_output(&stdout),
        stderr: truncate_output(&stderr),
    }
}

/// Run `job`'s handler, capturing its output, then append the resulting [`RunRecord`] to its
/// `runs.jsonl`.
///
/// When the handler does not exist there is no process to wait on, so [`execute_and_capture`]
/// returns immediately; that branch runs synchronously here (no thread) rather than `thread::spawn`,
/// keeping the common "handler not configured yet" trigger fast and avoiding a needless background
/// write racing an immediate caller-side `runs.jsonl`-directory removal (e.g. in tests). When the
/// handler exists, capture genuinely waits on the child process, so it runs on a background thread
/// so the caller (a manual-trigger HTTP/MCP handler) stays non-blocking, matching
/// [`crate::utils::process::spawn_and_reap`]'s fire-and-forget contract. The returned [`JoinHandle`]
/// lets tests await completion deterministically; production callers drop it.
pub fn spawn_capture_and_append(job: CronJob, trigger: RunTrigger) -> JoinHandle<()> {
    let handler_path = crate::paths::handlers_dir().join(&job.handler);
    if handler_path.exists() {
        thread::spawn(move || capture_and_append(&job, &handler_path, trigger))
    } else {
        capture_and_append(&job, &handler_path, trigger);
        thread::spawn(|| ())
    }
}

/// Run [`execute_and_capture`] for `job` against `handler_path`, then append the resulting record
/// to its `runs.jsonl`, logging (not propagating) any persistence failure.
fn capture_and_append(job: &CronJob, handler_path: &std::path::Path, trigger: RunTrigger) {
    let record = execute_and_capture(job, handler_path, trigger);
    if let Err(err) = append_run(&job.id, &record) {
        log::warn!("failed to persist run record for job {}: {err}", job.id);
    }
}

/// Append `record` to job `job_id`'s `runs.jsonl`, trimming to the most recent
/// [`MAX_RUNS_PER_JOB`] entries (oldest dropped first) and creating the job directory if it does
/// not exist yet.
pub fn append_run(job_id: &str, record: &RunRecord) -> std::io::Result<()> {
    let mut records = load_runs_raw(job_id)?;
    records.push(record.clone());
    if records.len() > MAX_RUNS_PER_JOB {
        let overflow = records.len() - MAX_RUNS_PER_JOB;
        records.drain(0..overflow);
    }
    std::fs::create_dir_all(crate::paths::job_dir(job_id))?;
    let mut body = String::new();
    for entry in &records {
        body.push_str(&serde_json::to_string(entry).unwrap_or_default());
        body.push('\n');
    }
    std::fs::write(crate::paths::job_runs_path(job_id), body)
}

/// Load all run records for `job_id` from disk in stored order (oldest first), skipping any line
/// that fails to parse (e.g. a partially written line from a crash). Returns an empty vec if the
/// file does not exist yet.
fn load_runs_raw(job_id: &str) -> std::io::Result<Vec<RunRecord>> {
    let path = crate::paths::job_runs_path(job_id);
    if !path.exists() {
        return Ok(Vec::new());
    }
    let content = std::fs::read_to_string(&path)?;
    Ok(content
        .lines()
        .filter_map(|line| serde_json::from_str::<RunRecord>(line).ok())
        .collect())
}

/// Load `job_id`'s run records, most-recent first, capped at [`MAX_RUNS_PER_JOB`] entries.
pub fn load_runs(job_id: &str) -> std::io::Result<Vec<RunRecord>> {
    let mut records = load_runs_raw(job_id)?;
    records.reverse();
    records.truncate(MAX_RUNS_PER_JOB);
    Ok(records)
}

#[cfg(test)]
#[path = "runs_tests.rs"]
mod runs_tests;
