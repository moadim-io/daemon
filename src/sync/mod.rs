//! Synchronization from moadim managed jobs into the OS crontab.
//!
//! Moadim owns a single delimited block inside the user's crontab:
//!
//! ```text
//! # BEGIN MOADIM
//! # Managed by moadim — edits here are overwritten on the next sync
//! 30 9 * * 1-5 /home/user/.config/moadim/handlers/send-report # moadim:uuid
//! # END MOADIM
//! ```
//!
//! **Forward sync** (moadim → crontab): called after every job mutation.
//! Enabled managed jobs are written into the block; disabled/deleted jobs are removed.
//! This is the only sync direction the daemon runs.
//!
//! **Reverse sync** (crontab → moadim) is *not* wired up. The functions that
//! implement it ([`sync_from_crontab`](crate::sync::sync_from_crontab) and its `parse_block` /
//! `parse_moadim_line` / `to_moadim_schedule` / `handler_from_command` helpers)
//! exist and are unit-tested, but no caller invokes them on any interval or at
//! startup — see issue #218. As a result, manual edits to the block do **not**
//! round-trip back into the store: a hand-edited schedule or handler is
//! reverted by the next forward sync, and hand-added lines are never imported.
//! These helpers are kept (behind `#[allow(dead_code)]`) so reverse sync can be
//! enabled later without re-deriving the parser.

use crate::utils::lock::LockRecover;
use std::collections::{HashMap, HashSet};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::cron_jobs::{CronJob, CronStore};
use crate::paths::handlers_dir;
use crate::storage::write_job;
use crate::utils::time::now_secs;

/// Crontab block for routines (agent-driven tmux jobs).
pub mod routines;

/// Delimiter marking the start of the moadim-owned crontab block.
const BLOCK_BEGIN: &str = "# BEGIN MOADIM";
/// Delimiter marking the end of the moadim-owned crontab block.
const BLOCK_END: &str = "# END MOADIM";
/// Human-readable header comment written inside the block.
///
/// Reverse sync is not wired up (see the module docs and issue #218), so manual
/// edits to this block do not round-trip — they are overwritten by the next
/// forward sync. The header says so rather than promising automatic sync-back.
const BLOCK_HEADER: &str = "# Managed by moadim — edits here are overwritten on the next sync";

// ─── Error type ────────────────────────────────────────────────────────────

/// Error returned by crontab sync operations.
#[derive(Debug)]
pub enum SyncError {
    /// The `crontab` command failed or was not found.
    CrontabCommand(String),
    /// An I/O error occurred while persisting a job.
    Io(std::io::Error),
}

impl std::fmt::Display for SyncError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SyncError::CrontabCommand(msg) => write!(f, "crontab: {msg}"),
            SyncError::Io(err) => write!(f, "io: {err}"),
        }
    }
}

impl From<std::io::Error> for SyncError {
    fn from(err: std::io::Error) -> Self {
        SyncError::Io(err)
    }
}

// ─── Schedule conversion ───────────────────────────────────────────────────

/// Convert a moadim schedule to a 5-field OS crontab schedule
/// (`min hour dom month dow`).
///
/// `croner` accepts both 6-field (`sec min hour dom month dow`) and 7-field
/// (`sec min hour dom month dow year`) forms. Both carry a leading seconds field
/// the OS crontab cannot express, so it is dropped (along with the trailing
/// year), projecting onto 5 fields. `@keyword` and already-5-field schedules are
/// passed through unchanged.
pub(crate) fn to_os_schedule(schedule: &str) -> String {
    let trimmed = schedule.trim();
    if trimmed.starts_with('@') {
        return trimmed.to_string();
    }
    let fields: Vec<&str> = trimmed.split_ascii_whitespace().collect();
    match fields.len() {
        6 | 7 => fields[1..6].join(" "),
        _ => trimmed.to_string(),
    }
}

/// Normalize a schedule read from the OS crontab into moadim's stored format.
///
/// Moadim uses 5-field format (`min hour dom month dow`) natively, which is the
/// same as the OS crontab. The value is returned as-is. `@keyword` schedules are
/// also passed through unchanged.
#[allow(dead_code)]
pub(crate) fn to_moadim_schedule(schedule: &str) -> String {
    schedule.trim().to_string()
}

// ─── Handler path resolution ───────────────────────────────────────────────

/// Resolve `handler` to a full path under `dir`, trying an exact match first
/// then common script extensions.
pub(crate) fn resolve_handler_path(handler: &str, dir: &Path) -> PathBuf {
    let exact = dir.join(handler);
    if exact.exists() {
        return exact;
    }
    for ext in ["sh", "py", "js", "rb", "pl", "bash", "zsh"] {
        let candidate = dir.join(format!("{handler}.{ext}"));
        if candidate.exists() {
            return candidate;
        }
    }
    // Return the name-without-extension path even if it does not exist yet.
    exact
}

/// Derive the handler name from a full command string.
///
/// If `command` is a path under `dir`, the stem (filename without extension)
/// is used. Otherwise the bare filename stem is returned.
#[allow(dead_code)]
pub(crate) fn handler_from_command(command: &str, dir: &Path) -> String {
    let path = Path::new(command.trim());
    let stem = if let Ok(rel) = path.strip_prefix(dir) {
        rel.file_stem()
            .and_then(|stem| stem.to_str())
            .map(str::to_string)
    } else {
        path.file_stem()
            .and_then(|stem| stem.to_str())
            .map(str::to_string)
    };
    stem.unwrap_or_else(|| command.trim().to_string())
}

// ─── Crontab I/O ──────────────────────────────────────────────────────────

/// Resolve the `crontab` binary to invoke.
///
/// Honours the `MOADIM_CRONTAB_BIN` environment variable when set, falling back
/// to the system `crontab` otherwise. The override exists so tests can point
/// crontab I/O at a shim instead of mutating the developer's real crontab.
///
/// In **test builds**, when no `MOADIM_CRONTAB_BIN` shim is configured this never
/// falls back to the real system `crontab`: it returns a path that cannot exist,
/// so the spawn fails and the sync logs a warning instead of clobbering the
/// developer's live crontab. This is a structural safety net for issue #175 — a
/// test that forgets to install a shim (or clear `PATH`) still cannot touch the
/// real crontab. Tests that need a working sync set `MOADIM_CRONTAB_BIN` to a
/// shim, which is honoured first.
fn crontab_bin() -> String {
    if let Ok(bin) = std::env::var("MOADIM_CRONTAB_BIN") {
        return bin;
    }
    #[cfg(test)]
    let fallback = "/nonexistent/moadim-test-crontab-guard".to_string();
    #[cfg(not(test))]
    let fallback = "crontab".to_string();
    fallback
}

/// Read the current user crontab via `crontab -l`.
///
/// Returns an empty string when no crontab exists for the user.
pub(crate) fn read_crontab() -> Result<String, SyncError> {
    let out = Command::new(crontab_bin())
        .arg("-l")
        .output()
        .map_err(|err| SyncError::CrontabCommand(format!("failed to run crontab -l: {err}")))?;

    if out.status.success() {
        return Ok(String::from_utf8_lossy(&out.stdout).into_owned());
    }
    // "no crontab for <user>" is a normal condition — treat as empty.
    let stderr = String::from_utf8_lossy(&out.stderr);
    if stderr.contains("no crontab") {
        return Ok(String::new());
    }
    Err(SyncError::CrontabCommand(stderr.into_owned()))
}

/// Install `content` as the user's crontab via `crontab -`.
pub(crate) fn write_crontab(content: &str) -> Result<(), SyncError> {
    let mut child = Command::new(crontab_bin())
        .arg("-")
        .stdin(Stdio::piped())
        .spawn()
        .map_err(|err| SyncError::CrontabCommand(format!("failed to spawn crontab: {err}")))?;

    // Taking stdin drops (closes) it after write_all, signalling EOF.
    child
        .stdin
        .take()
        .expect("stdin is piped")
        .write_all(content.as_bytes())?;

    let status = child.wait()?;

    if !status.success() {
        return Err(SyncError::CrontabCommand(format!(
            "crontab - exited with {status}"
        )));
    }
    Ok(())
}

// ─── Block assembly ────────────────────────────────────────────────────────

/// Format a single managed job as a crontab line with a trailing `# moadim:<id>` tag.
pub(crate) fn format_crontab_line(job: &CronJob, handlers: &Path) -> String {
    let schedule = to_os_schedule(&job.schedule);
    let path = resolve_handler_path(&job.handler, handlers);
    format!("{} {} # moadim:{}", schedule, path.display(), job.id)
}

/// Build the full moadim block string from the enabled managed jobs in `store`.
///
/// Only jobs assigned to *this* machine ([`crate::machine::current_machine`]) are scheduled, so a
/// shared config repo can drive different jobs on different machines. A job with an empty `machines`
/// list runs nowhere and is logged once as dormant (see [`warn_dormant_jobs`]).
fn build_block(store: &CronStore) -> String {
    let dir = handlers_dir();
    let me = crate::machine::current_machine();
    let mut jobs: Vec<CronJob> = {
        let lock = store.lock_recover();
        lock.values()
            .filter(|j| j.source == "managed" && j.enabled)
            .cloned()
            .collect()
    };
    warn_dormant_jobs(&jobs);
    jobs.retain(|j| crate::machine::targets(&j.machines, &me));
    jobs.sort_by_key(|j| j.created_at);

    let lines: Vec<String> = jobs.iter().map(|j| format_crontab_line(j, &dir)).collect();

    if lines.is_empty() {
        format!("{BLOCK_BEGIN}\n{BLOCK_HEADER}\n{BLOCK_END}")
    } else {
        format!(
            "{BLOCK_BEGIN}\n{BLOCK_HEADER}\n{}\n{BLOCK_END}",
            lines.join("\n")
        )
    }
}

/// Log a single warning naming enabled managed jobs with no machine assignment (empty `machines`).
///
/// Mirrors the routine dormant-warning: with "unset targeting = runs nowhere", such jobs never
/// schedule on any machine, so surfacing them once at sync time keeps the upgrade-goes-dormant
/// behavior visible instead of silent.
fn warn_dormant_jobs(jobs: &[CronJob]) {
    let dormant: Vec<&str> = jobs
        .iter()
        .filter(|j| j.machines.is_empty())
        .map(|j| j.id.as_str())
        .collect();
    if !dormant.is_empty() {
        log::warn!(
            "{} enabled cron job(s) have no machine assignment and will not be scheduled on any \
             machine: {}; assign with `moadim cron-jobs update <id> --machines '[\"<name>\"]'`",
            dormant.len(),
            dormant.join(", ")
        );
    }
}

/// Replace (or insert) the moadim handler block inside `crontab` text.
pub(crate) fn replace_block(crontab: &str, block: &str) -> String {
    replace_block_with(crontab, block, BLOCK_BEGIN, BLOCK_END)
}

/// Replace (or insert) a delimited block (`begin_marker`..`end_marker`) inside `crontab` text.
pub(crate) fn replace_block_with(
    crontab: &str,
    block: &str,
    begin_marker: &str,
    end_marker: &str,
) -> String {
    let begin_pos = crontab.find(begin_marker);
    let end_pos = crontab.find(end_marker);

    match (begin_pos, end_pos) {
        (Some(begin), Some(end)) if begin < end => {
            let after = end + end_marker.len();
            let mut result = crontab[..begin].to_string();
            result.push_str(block);
            result.push('\n');
            let rest = crontab[after..].trim_start_matches('\n');
            if !rest.is_empty() {
                result.push('\n');
                result.push_str(rest);
                // Preserve trailing newline from original if present.
                if !result.ends_with('\n') {
                    result.push('\n');
                }
            }
            result
        }
        (Some(begin), _) => {
            // Malformed block (begin without end): replace from begin to end of string.
            let mut result = crontab[..begin].to_string();
            result.push_str(block);
            result.push('\n');
            result
        }
        _ => {
            // No existing block — append after existing content.
            let mut result = crontab.trim_end_matches('\n').to_string();
            if !result.is_empty() {
                result.push('\n');
            }
            result.push_str(block);
            result.push('\n');
            result
        }
    }
}

// ─── Block parsing ─────────────────────────────────────────────────────────

/// Parse a crontab line that carries a `# moadim:<uuid>` tag.
///
/// Returns `(uuid, os_schedule, command)` on success.
#[allow(dead_code)]
pub(crate) fn parse_moadim_line(line: &str) -> Option<(String, String, String)> {
    let tag = "# moadim:";
    let comment_pos = line.rfind(tag)?;
    let uuid = line[comment_pos + tag.len()..].trim().to_string();
    if uuid.is_empty() {
        return None;
    }
    let body = line[..comment_pos].trim();

    if body.starts_with('@') {
        // @keyword command
        let mut parts = body.splitn(2, |ch: char| ch.is_ascii_whitespace());
        let schedule = parts.next()?.trim().to_string();
        let command = parts.next()?.trim().to_string();
        return Some((uuid, schedule, command));
    }

    // Standard 5-field: min hour dom month dow command...
    let tokens: Vec<&str> = body.split_ascii_whitespace().collect();
    if tokens.len() < 6 {
        return None;
    }
    let schedule = tokens[..5].join(" ");
    let command = tokens[5..].join(" ");
    Some((uuid, schedule, command))
}

/// Extract all moadim entries from a crontab string.
///
/// Returns a map of `uuid → (os_schedule, command)`.
#[allow(dead_code)]
pub(crate) fn parse_block(crontab: &str) -> HashMap<String, (String, String)> {
    let mut in_block = false;
    let mut entries = HashMap::new();

    for line in crontab.lines() {
        let trimmed = line.trim();
        if trimmed == BLOCK_BEGIN {
            in_block = true;
            continue;
        }
        if trimmed == BLOCK_END {
            break;
        }
        if !in_block || trimmed.starts_with('#') || trimmed.is_empty() {
            continue;
        }
        if let Some((id, sched, cmd)) = parse_moadim_line(line) {
            entries.insert(id, (sched, cmd));
        }
    }

    entries
}

// ─── Public sync API ───────────────────────────────────────────────────────

/// Write all enabled managed jobs from `store` into the OS crontab block.
///
/// Idempotent: skips the `crontab -` call when the crontab would not change.
/// Call this after every job mutation. Errors are logged by the caller.
pub fn sync_to_crontab(store: &CronStore) -> Result<(), SyncError> {
    let current = read_crontab()?;
    let block = build_block(store);
    let new_crontab = replace_block(&current, &block);
    if new_crontab == current {
        return Ok(());
    }
    write_crontab(&new_crontab)
}

/// Read the OS crontab and reconcile changes in the moadim block back into `store`.
///
/// - Jobs whose schedule or handler changed in the block are updated in memory
///   and persisted to TOML.
/// - Lines with an unknown UUID are imported as new managed jobs.
/// - Jobs present in the store but absent from the block are left unchanged
///   (absence means the job is disabled, so it was intentionally excluded from
///   the block by the last forward sync).
///
/// Returns `true` if any jobs were created or updated.
#[allow(dead_code)]
pub fn sync_from_crontab(store: &CronStore) -> Result<bool, SyncError> {
    let crontab = read_crontab()?;
    let block_entries = parse_block(&crontab);
    let dir = handlers_dir();
    let now = now_secs();

    let mut jobs_to_write: Vec<CronJob> = Vec::new();
    let mut changed = false;

    {
        let mut lock = store.lock_recover();

        // Update existing managed jobs whose schedule or handler changed in the block.
        for job in lock.values_mut().filter(|j| j.source == "managed") {
            if let Some((os_sched, command)) = block_entries.get(&job.id) {
                let new_schedule = to_moadim_schedule(os_sched);
                let new_handler = handler_from_command(command, &dir);

                let sched_changed = new_schedule != job.schedule;
                let handler_changed = new_handler != job.handler;

                if sched_changed || handler_changed {
                    if sched_changed {
                        job.schedule = new_schedule;
                    }
                    if handler_changed {
                        job.handler = new_handler;
                    }
                    job.updated_at = now;
                    jobs_to_write.push(job.clone());
                    changed = true;
                }
            }
        }

        // Import entries whose UUID is not known to the store.
        let known: HashSet<String> = lock.keys().cloned().collect();
        for (id, (os_sched, command)) in &block_entries {
            if !known.contains(id) {
                let job = CronJob {
                    id: id.clone(),
                    schedule: to_moadim_schedule(os_sched),
                    handler: handler_from_command(command, &dir),
                    metadata: serde_json::json!({}),
                    machines: Vec::new(),
                    enabled: true,
                    source: "managed".to_string(),
                    created_at: now,
                    updated_at: now,
                    last_manual_trigger_at: None,
                };
                lock.insert(id.clone(), job.clone());
                jobs_to_write.push(job);
                changed = true;
            }
        }
    }

    // Persist changes outside the lock.
    for job in &jobs_to_write {
        if let Err(err) = write_job(job) {
            log::warn!("cron_sync: failed to persist job {}: {err}", job.id);
        }
    }

    Ok(changed)
}

#[cfg(test)]
#[path = "cron_sync_tests.rs"]
mod cron_sync_tests;
