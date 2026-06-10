//! TOML-backed persistence for managed cron jobs.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};

use crate::cron_jobs::{CronJob, CronStore};
use crate::paths::{job_dir, job_gitignore_path, job_local_toml_path, job_toml_path, jobs_dir};

/// TOML representation of a job on disk. `job.local.toml` may override any field.
#[derive(Debug, Deserialize, Serialize)]
struct JobToml {
    /// Cron expression.
    schedule: Option<String>,
    /// Handler identifier.
    handler: Option<String>,
    /// Whether the job is enabled.
    enabled: Option<bool>,
    /// Unix creation timestamp.
    created_at: Option<u64>,
    /// Unix last-updated timestamp.
    updated_at: Option<u64>,
    /// Unix timestamp of last manual trigger.
    last_triggered_at: Option<u64>,
    /// Arbitrary metadata key/value pairs.
    #[serde(default)]
    metadata: toml::Table,
}

/// Parse a TOML file at `path`, returning `None` on any error.
fn read_job_toml(path: &std::path::PathBuf) -> Option<JobToml> {
    let text = std::fs::read_to_string(path).ok()?;
    toml::from_str(&text).ok()
}

/// Convert a TOML table to a JSON object value.
fn metadata_to_json(table: &toml::Table) -> serde_json::Value {
    serde_json::to_value(table).unwrap_or(serde_json::Value::Object(Default::default()))
}

/// Convert a JSON object value to a TOML table, skipping non-representable values.
fn json_to_toml_table(val: &serde_json::Value) -> toml::Table {
    match val {
        serde_json::Value::Object(map) => {
            let mut table = toml::Table::new();
            for (k, v) in map {
                if let Ok(tv) = serde_json::from_value::<toml::Value>(v.clone()) {
                    table.insert(k.clone(), tv);
                }
            }
            table
        }
        _ => toml::Table::new(),
    }
}

/// Load a managed job from `{jobs_dir}/{id}/`, merging `job.local.toml` overrides.
fn load_job_from_dir(id: &str) -> Option<CronJob> {
    let base = read_job_toml(&job_toml_path(id))?;
    let local = read_job_toml(&job_local_toml_path(id));
    let (schedule, handler, enabled, created_at, updated_at, last_triggered_at, mut meta) = (
        local.as_ref().and_then(|l| l.schedule.clone()).or(base.schedule)?,
        local.as_ref().and_then(|l| l.handler.clone()).or(base.handler)?,
        local.as_ref().and_then(|l| l.enabled).or(base.enabled).unwrap_or(true),
        local.as_ref().and_then(|l| l.created_at).or(base.created_at).unwrap_or(0),
        local.as_ref().and_then(|l| l.updated_at).or(base.updated_at).unwrap_or(0),
        local.as_ref().and_then(|l| l.last_triggered_at).or(base.last_triggered_at),
        base.metadata,
    );
    if let Some(local_meta) = local.as_ref().map(|l| &l.metadata) {
        for (k, v) in local_meta {
            meta.insert(k.clone(), v.clone());
        }
    }
    Some(CronJob {
        id: id.to_string(),
        schedule,
        handler,
        enabled,
        source: "managed".to_string(),
        created_at,
        updated_at,
        last_triggered_at,
        metadata: metadata_to_json(&meta),
    })
}

/// Write `job` to `{jobs_dir}/{job.id}/job.toml`, creating the directory and `.gitignore` if needed.
pub fn write_job(job: &CronJob) -> std::io::Result<()> {
    let dir = job_dir(&job.id);
    std::fs::create_dir_all(&dir)?;

    let gitignore = job_gitignore_path(&job.id);
    if !gitignore.exists() {
        std::fs::write(&gitignore, "*.local.*\n*.log\n")?;
    }

    let toml_job = JobToml {
        schedule: Some(job.schedule.clone()),
        handler: Some(job.handler.clone()),
        enabled: Some(job.enabled),
        created_at: Some(job.created_at),
        updated_at: Some(job.updated_at),
        last_triggered_at: job.last_triggered_at,
        metadata: json_to_toml_table(&job.metadata),
    };
    let text = toml::to_string_pretty(&toml_job)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    std::fs::write(job_toml_path(&job.id), text)?;
    Ok(())
}

/// Remove the directory for job `id`, doing nothing if it does not exist.
pub fn remove_job_dir(id: &str) -> std::io::Result<()> {
    let dir = job_dir(id);
    if dir.exists() {
        std::fs::remove_dir_all(dir)?;
    }
    Ok(())
}

/// Scan `~/.config/moadim/jobs/` and load all valid managed jobs into a new store.
pub fn load_store() -> CronStore {
    let dir = jobs_dir();
    let mut jobs = HashMap::new();
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                let id = entry.file_name().to_string_lossy().to_string();
                if let Some(job) = load_job_from_dir(&id) {
                    jobs.insert(id, job);
                }
            }
        }
    }
    Arc::new(Mutex::new(jobs))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metadata_roundtrip() {
        let val = serde_json::json!({"key": "value", "num": 42});
        let table = json_to_toml_table(&val);
        let back = metadata_to_json(&table);
        assert_eq!(back["key"], "value");
        assert_eq!(back["num"], 42);
    }
}
