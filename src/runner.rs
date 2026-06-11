//! Execution engine for managed cron jobs.
//!
//! The HTTP/MCP layers only persist job declarations; this module is what
//! actually runs them. [`spawn_scheduler`] launches a background task that
//! evaluates every enabled managed job's cron expression against the local
//! clock and invokes [`run_job`] when a schedule fires. [`run_job`] resolves
//! the job's handler to an executable under `~/.config/moadim/handlers/`,
//! passes job metadata as `MOADIM_*` environment variables, runs it, and
//! appends a start/finish record to the job's `job.log`.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::str::FromStr;
use std::time::{Duration, Instant};

use chrono::{Local, SecondsFormat};
use cron::Schedule;

use crate::cron_jobs::{CronJob, CronStore};
use crate::paths::{handlers_dir, job_log_path};

/// How often the scheduler wakes to check for due jobs.
const TICK: Duration = Duration::from_secs(1);

/// Source value identifying jobs this server owns and is responsible for running.
const MANAGED_SOURCE: &str = "managed";

/// Spawn the background scheduler task.
///
/// On every [`TICK`] it loads the current set of enabled, managed jobs from
/// `store` and runs any whose cron schedule has a fire time in the interval
/// since the previous tick. Each run is dispatched on its own task so a
/// long-running handler never blocks the scheduler or other jobs.
pub fn spawn_scheduler(store: CronStore) {
    tokio::spawn(async move {
        // Anchor the first window at "now" so jobs created before startup do
        // not all fire immediately on boot.
        let mut last = Local::now();
        loop {
            tokio::time::sleep(TICK).await;
            let now = Local::now();

            let due: Vec<CronJob> = {
                let lock = store.lock().unwrap();
                lock.values()
                    .filter(|j| j.enabled && j.source == MANAGED_SOURCE)
                    .filter(|j| match Schedule::from_str(&j.schedule) {
                        // Fire if the next scheduled time strictly after `last`
                        // falls at or before `now` (i.e. inside this window).
                        Ok(sched) => sched.after(&last).next().is_some_and(|t| t <= now),
                        Err(_) => false,
                    })
                    .cloned()
                    .collect()
            };

            for job in due {
                tokio::spawn(run_job(job));
            }

            last = now;
        }
    });
}

/// Run a single job's handler to completion, logging start and finish.
///
/// Resolves the handler name to an executable under [`handlers_dir`], injects
/// `MOADIM_*` environment variables derived from the job's id, handler,
/// schedule, and metadata, then executes it. A start line and a finish line
/// (with status and elapsed seconds) are appended to the job's `job.log`.
/// Missing handlers and spawn failures are recorded in the log rather than
/// propagated, since this runs detached from any request.
pub async fn run_job(job: CronJob) {
    append_log(&job.id, &format!("[{}] run started", job.handler));

    let handler_path = match resolve_handler(&job.handler) {
        Some(p) => p,
        None => {
            append_log(
                &job.id,
                &format!(
                    "[{}] run failed: handler not found in {}",
                    job.handler,
                    handlers_dir().display()
                ),
            );
            return;
        }
    };

    let mut cmd = tokio::process::Command::new(&handler_path);
    cmd.env("MOADIM_JOB_ID", &job.id)
        .env("MOADIM_HANDLER", &job.handler)
        .env("MOADIM_SCHEDULE", &job.schedule);
    for (key, value) in metadata_env(&job.metadata) {
        cmd.env(key, value);
    }

    let started = Instant::now();
    let result = cmd.output().await;
    let elapsed = started.elapsed().as_secs_f64();

    match result {
        Ok(output) if output.status.success() => {
            append_log(
                &job.id,
                &format!("[{}] run finished OK ({:.1}s)", job.handler, elapsed),
            );
        }
        Ok(output) => {
            append_log(
                &job.id,
                &format!(
                    "[{}] run finished with status {} ({:.1}s)",
                    job.handler, output.status, elapsed
                ),
            );
        }
        Err(e) => {
            append_log(
                &job.id,
                &format!("[{}] run failed to spawn: {} ({:.1}s)", job.handler, e, elapsed),
            );
        }
    }
}

/// Resolve a handler identifier to an executable path under [`handlers_dir`].
///
/// Matches an exact filename first (e.g. `handler = "send-report"` →
/// `handlers/send-report`), then any file whose stem matches the identifier
/// (e.g. `handlers/send-report.sh`). Returns `None` if nothing matches.
fn resolve_handler(handler: &str) -> Option<PathBuf> {
    let dir = handlers_dir();

    let exact = dir.join(handler);
    if exact.is_file() {
        return Some(exact);
    }

    let entries = std::fs::read_dir(&dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() && path.file_stem().and_then(|s| s.to_str()) == Some(handler) {
            return Some(path);
        }
    }
    None
}

/// Build `MOADIM_*` environment variables from a job's JSON metadata.
///
/// Each scalar value in a metadata object becomes `MOADIM_{UPPERCASED_KEY}`.
/// Non-scalar values (arrays, nested objects) and non-object metadata are
/// skipped. Keys are sorted for deterministic ordering.
fn metadata_env(metadata: &serde_json::Value) -> BTreeMap<String, String> {
    let mut env = BTreeMap::new();
    if let serde_json::Value::Object(map) = metadata {
        for (key, value) in map {
            let rendered = match value {
                serde_json::Value::String(s) => s.clone(),
                serde_json::Value::Number(n) => n.to_string(),
                serde_json::Value::Bool(b) => b.to_string(),
                _ => continue,
            };
            env.insert(format!("MOADIM_{}", key.to_uppercase()), rendered);
        }
    }
    env
}

/// Append a timestamped line to job `id`'s `job.log`, creating it if needed.
///
/// Errors are intentionally ignored: logging must never interrupt job
/// execution, and the job directory may legitimately not exist yet.
fn append_log(id: &str, message: &str) {
    use std::io::Write;

    let stamp = Local::now().to_rfc3339_opts(SecondsFormat::Secs, false);
    let line = format!("{stamp} {message}\n");

    let path = job_log_path(id);
    if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(&path) {
        let _ = file.write_all(line.as_bytes());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metadata_env_renders_scalars_and_skips_complex() {
        let meta = serde_json::json!({
            "recipient": "team@example.com",
            "retries": 3,
            "active": true,
            "tags": ["a", "b"],
            "nested": {"k": "v"}
        });
        let env = metadata_env(&meta);
        assert_eq!(env.get("MOADIM_RECIPIENT").map(String::as_str), Some("team@example.com"));
        assert_eq!(env.get("MOADIM_RETRIES").map(String::as_str), Some("3"));
        assert_eq!(env.get("MOADIM_ACTIVE").map(String::as_str), Some("true"));
        assert!(!env.contains_key("MOADIM_TAGS"));
        assert!(!env.contains_key("MOADIM_NESTED"));
    }

    #[test]
    fn metadata_env_empty_for_non_object() {
        assert!(metadata_env(&serde_json::Value::Null).is_empty());
    }

    #[test]
    fn resolve_handler_missing_is_none() {
        assert!(resolve_handler("definitely-not-a-real-handler-xyz").is_none());
    }

    /// End-to-end: `run_job` resolves the handler, passes `MOADIM_*` env, runs
    /// the process, and appends a success line to `job.log`. Drives a temp HOME
    /// so it touches no real config. Unix-only (uses a shell handler).
    #[cfg(unix)]
    #[tokio::test]
    async fn run_job_executes_handler_with_env_and_logs() {
        use std::os::unix::fs::PermissionsExt;

        let base = std::env::temp_dir().join(format!("moadim-runtest-{}", std::process::id()));
        let handlers = base.join(".config/moadim/handlers");
        std::fs::create_dir_all(&handlers).unwrap();
        // Pre-create the job dir so job.log has somewhere to land.
        std::fs::create_dir_all(base.join(".config/moadim/jobs/job-1")).unwrap();

        let marker = base.join("marker.out");
        let handler = handlers.join("marker");
        std::fs::write(
            &handler,
            format!(
                "#!/bin/sh\necho \"who=$MOADIM_WHO id=$MOADIM_JOB_ID\" > {}\n",
                marker.display()
            ),
        )
        .unwrap();
        std::fs::set_permissions(&handler, std::fs::Permissions::from_mode(0o755)).unwrap();

        // SAFETY: single-threaded async test; no other task reads HOME concurrently.
        unsafe {
            std::env::set_var("HOME", &base);
        }

        let job = CronJob {
            id: "job-1".to_string(),
            schedule: "0 * * * * *".to_string(),
            handler: "marker".to_string(),
            metadata: serde_json::json!({ "who": "scheduler" }),
            enabled: true,
            source: MANAGED_SOURCE.to_string(),
            created_at: 0,
            updated_at: 0,
            last_triggered_at: None,
        };
        run_job(job).await;

        let out = std::fs::read_to_string(&marker).expect("handler should have written marker");
        assert!(out.contains("who=scheduler"), "MOADIM_WHO not passed: {out}");
        assert!(out.contains("id=job-1"), "MOADIM_JOB_ID not passed: {out}");

        let log = std::fs::read_to_string(base.join(".config/moadim/jobs/job-1/job.log")).unwrap();
        assert!(log.contains("run started"), "missing start line: {log}");
        assert!(log.contains("run finished OK"), "missing finish line: {log}");

        let _ = std::fs::remove_dir_all(&base);
    }
}
