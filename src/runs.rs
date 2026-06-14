//! Per-run execution records.
//!
//! Each time a managed job fires — whether by the scheduler or a manual trigger —
//! a [`RunRecord`] is appended to `~/.config/moadim/jobs/{id}/runs.jsonl`.
//! The file is a newline-delimited JSON stream; each line is one compact record.
//! At most [`MAX_RUNS`] entries are retained; older ones are trimmed on every write.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::paths::job_runs_path;

/// Maximum number of run records kept per job.
const MAX_RUNS: usize = 100;

/// Per-stream capture limit in bytes.
const MAX_OUTPUT_BYTES: usize = 10 * 1024;

/// What initiated a job run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum RunTrigger {
    /// Fired automatically by the cron scheduler.
    Scheduled,
    /// Fired manually via the REST or MCP trigger endpoint.
    Manual,
}

/// A record of a single job execution.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, utoipa::ToSchema)]
pub struct RunRecord {
    /// Unique identifier for this run (UUID v4).
    pub id: String,
    /// ID of the job that ran.
    pub job_id: String,
    /// Unix timestamp (seconds) when the run started.
    pub started_at: u64,
    /// Unix timestamp (seconds) when the run finished.
    pub finished_at: u64,
    /// Wall-clock duration of the run in milliseconds.
    pub duration_ms: u64,
    /// Process exit code, or `None` if the handler could not be spawned.
    pub exit_code: Option<i32>,
    /// Captured standard output (truncated to 10 KB if larger).
    pub stdout: String,
    /// Captured standard error (truncated to 10 KB if larger).
    pub stderr: String,
    /// What initiated this run.
    pub trigger: RunTrigger,
}

impl RunRecord {
    /// Construct a new record. Raw stdout/stderr bytes are decoded as UTF-8
    /// (lossy) and truncated to [`MAX_OUTPUT_BYTES`] if they exceed that limit.
    pub fn new(
        job_id: &str,
        started_at: u64,
        finished_at: u64,
        duration_ms: u64,
        exit_code: Option<i32>,
        stdout: Vec<u8>,
        stderr: Vec<u8>,
        trigger: RunTrigger,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            job_id: job_id.to_string(),
            started_at,
            finished_at,
            duration_ms,
            exit_code,
            stdout: truncate_output(stdout),
            stderr: truncate_output(stderr),
            trigger,
        }
    }
}

/// Append `record` to the job's `runs.jsonl`, retaining at most [`MAX_RUNS`] entries.
///
/// Errors are intentionally ignored — run persistence must never interrupt job
/// execution or block request handling.
pub fn append_run(record: &RunRecord) {
    let path = job_runs_path(&record.job_id);
    let mut runs = load_from_path(&path);
    runs.push(record.clone());
    if runs.len() > MAX_RUNS {
        runs.drain(0..runs.len() - MAX_RUNS);
    }
    let content = runs
        .iter()
        .filter_map(|r| serde_json::to_string(r).ok())
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    let _ = std::fs::write(&path, content);
}

/// Return all run records for `job_id`, most-recent first.
pub fn load_runs(job_id: &str) -> Vec<RunRecord> {
    let mut runs = load_from_path(&job_runs_path(job_id));
    runs.reverse();
    runs
}

fn load_from_path(path: &std::path::Path) -> Vec<RunRecord> {
    std::fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect()
}

fn truncate_output(bytes: Vec<u8>) -> String {
    let s = String::from_utf8_lossy(&bytes).into_owned();
    if s.len() > MAX_OUTPUT_BYTES {
        let tail_start = s.len() - MAX_OUTPUT_BYTES;
        format!(
            "[truncated — {} bytes total, showing last 10 KB]\n{}",
            s.len(),
            &s[tail_start..]
        )
    } else {
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_short_string_unchanged() {
        let bytes = b"hello world".to_vec();
        assert_eq!(truncate_output(bytes), "hello world");
    }

    #[test]
    fn truncate_long_string_adds_header() {
        let big = vec![b'x'; MAX_OUTPUT_BYTES + 1];
        let out = truncate_output(big);
        assert!(out.contains("[truncated"));
        // Header + tail should not be wildly larger than the limit
        assert!(out.len() < MAX_OUTPUT_BYTES + 200);
    }

    #[test]
    fn append_and_load_roundtrip() {
        let dir = std::env::temp_dir().join(format!("moadim-runs-rt-{}", std::process::id()));
        let job_id = "rt-job";
        std::fs::create_dir_all(dir.join(".config/moadim/jobs").join(job_id)).unwrap();
        // SAFETY: single-threaded test.
        unsafe {
            std::env::set_var("HOME", &dir);
        }

        let rec = RunRecord::new(
            job_id,
            1_000,
            1_001,
            500,
            Some(0),
            b"out".to_vec(),
            b"err".to_vec(),
            RunTrigger::Scheduled,
        );
        append_run(&rec);

        let runs = load_runs(job_id);
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].job_id, job_id);
        assert_eq!(runs[0].stdout, "out");
        assert_eq!(runs[0].stderr, "err");
        assert_eq!(runs[0].exit_code, Some(0));
        assert_eq!(runs[0].trigger, RunTrigger::Scheduled);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn append_trims_to_max_runs() {
        let dir = std::env::temp_dir().join(format!("moadim-runs-trim-{}", std::process::id()));
        let job_id = "trim-job";
        std::fs::create_dir_all(dir.join(".config/moadim/jobs").join(job_id)).unwrap();
        // SAFETY: single-threaded test.
        unsafe {
            std::env::set_var("HOME", &dir);
        }

        for i in 0..(MAX_RUNS + 5) {
            append_run(&RunRecord::new(
                job_id,
                i as u64,
                i as u64 + 1,
                10,
                Some(0),
                vec![],
                vec![],
                RunTrigger::Manual,
            ));
        }

        let runs = load_runs(job_id);
        assert_eq!(runs.len(), MAX_RUNS);
        // Most recent is first after reverse
        assert_eq!(runs[0].started_at, (MAX_RUNS + 4) as u64);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_runs_missing_file_is_empty() {
        let dir = std::env::temp_dir().join(format!("moadim-runs-missing-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        // SAFETY: single-threaded test.
        unsafe {
            std::env::set_var("HOME", &dir);
        }
        assert!(load_runs("no-such-job").is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
