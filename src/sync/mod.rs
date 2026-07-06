//! Synchronization from moadim managed routines into the OS crontab.
//!
//! Moadim owns a single delimited block inside the user's crontab for routines:
//!
//! ```text
//! # BEGIN MOADIM-ROUTINES
//! # Managed by moadim — routines (agent tmux sessions)
//! * * * * * /home/user/.local/bin/moadim schedule trigger '<id>' # moadim-routine:<id>
//! # END MOADIM-ROUTINES
//! ```
//!
//! **Forward sync** (moadim → crontab): called after every routine mutation.
//! Enabled managed routines are written into the block; disabled/deleted routines are removed.
//! This is the only sync direction the daemon runs. See [`crate::sync::routines::sync_routines_to_crontab`].

use std::io::Write;
use std::process::{Command, Stdio};

/// Crontab block for routines (agent-driven tmux jobs).
pub mod routines;

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
            Self::CrontabCommand(msg) => write!(f, "crontab: {msg}"),
            Self::Io(err) => write!(f, "io: {err}"),
        }
    }
}

impl From<std::io::Error> for SyncError {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err)
    }
}

// ─── Schedule conversion ───────────────────────────────────────────────────

/// Convert a 6-field (`sec min hour dom month dow`) or 7-field
/// (`sec min hour dom month dow year`) moadim schedule to a 5-field OS crontab
/// schedule (`min hour dom month dow`).
///
/// `@keyword` schedules are passed through unchanged. Both the 6- and 7-field
/// forms carry a leading seconds field, so field 0 (and, for the 7-field form,
/// the trailing year) is dropped. A 6-field schedule that is not reduced would
/// be written verbatim to the crontab where it is malformed and silently never
/// fires.
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

    // Taking stdin drops (closes) it after write_all, signalling EOF. A write
    // failure (e.g. the child exits early — a strict `crontab` rejecting
    // malformed input mid-stream — and closes its end of the pipe) is a real,
    // externally-triggerable I/O condition, not a programmer error, so it is
    // propagated as `SyncError::Io` instead of panicking: every caller of
    // crontab sync already treats a `SyncError` as warn-and-continue (see the
    // module docs), and a panic here would defeat that graceful degradation.
    let write_result = child
        .stdin
        .take()
        .expect("stdin is piped")
        .write_all(content.as_bytes());

    // Always wait() to reap the child, even when the write above failed.
    let status = child.wait()?;
    write_result?;

    if !status.success() {
        return Err(SyncError::CrontabCommand(format!(
            "crontab - exited with {status}"
        )));
    }
    Ok(())
}

// ─── Block assembly ────────────────────────────────────────────────────────

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

// ─── Public sync API ───────────────────────────────────────────────────────

/// Remove the managed routines crontab block (`# BEGIN MOADIM-ROUTINES`) from the user's
/// crontab, leaving every other entry untouched. Returns the number of managed schedule
/// lines removed.
///
/// Used by `moadim uninstall`: install registers an OS service *and* sync writes this
/// crontab block, so a clean teardown must clear it — otherwise `cron` keeps firing
/// routines against a daemon the user removed.
///
/// Best-effort and idempotent: a crontab with no managed block (or no crontab at all)
/// removes nothing and returns `0` without rewriting the crontab.
pub fn clear_managed_crontab_blocks() -> Result<usize, SyncError> {
    let current = read_crontab()?;

    // Count managed schedule lines before removal for the user-facing report.
    let removed = current
        .lines()
        .filter(|line| line.contains(routines::ROUTINE_LINE_MARKER))
        .count();

    if !current.contains(routines::BLOCK_BEGIN) {
        return Ok(0);
    }
    // No `updated == current` idempotency check here (contrast `sync_to_crontab`/
    // `sync_routines_to_crontab`): given the `BLOCK_BEGIN` guard above, `replace_block_with`
    // always strips at least the begin marker from `current`, so `updated` can never equal
    // `current` — the guard above is what makes repeated calls idempotent (a second call sees
    // no `BLOCK_BEGIN` and returns `Ok(0)` before reaching this point).
    let updated = replace_block_with(&current, "", routines::BLOCK_BEGIN, routines::BLOCK_END);
    write_crontab(&updated)?;
    Ok(removed)
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod sync_tests;

#[cfg(test)]
#[path = "mod_replace_block_tests.rs"]
mod replace_block_tests;
