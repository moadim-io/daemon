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
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

use crate::utils::lock::LockRecover;
#[cfg(any(not(target_os = "macos"), test))]
use crate::utils::time::now_secs;

#[allow(
    clippy::missing_docs_in_private_items,
    reason = "split-out module keeps the file under the linecheck limit"
)]
mod wait_for_crontab_write;
pub(crate) use wait_for_crontab_write::*;

/// Snapshot of the most recent OS crontab sync result.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CrontabSyncStatus {
    /// Whether the most recent sync attempt completed successfully.
    pub ok: bool,
    /// Last sync error, when the most recent attempt failed.
    pub last_error: Option<String>,
    /// Unix timestamp of the last sync failure, when known.
    pub last_error_at: Option<u64>,
}

impl Default for CrontabSyncStatus {
    fn default() -> Self {
        Self {
            ok: true,
            last_error: None,
            last_error_at: None,
        }
    }
}

/// Process-local health state for OS crontab sync.
fn crontab_sync_state() -> &'static Mutex<CrontabSyncStatus> {
    static STATE: OnceLock<Mutex<CrontabSyncStatus>> = OnceLock::new();
    STATE.get_or_init(|| Mutex::new(CrontabSyncStatus::default()))
}

/// Return the last recorded OS crontab sync status.
pub(crate) fn crontab_sync_status() -> CrontabSyncStatus {
    crontab_sync_state().lock_recover().clone()
}

/// Mark the OS crontab sync state healthy after a successful sync attempt.
pub(crate) fn record_crontab_sync_success() {
    *crontab_sync_state().lock_recover() = CrontabSyncStatus::default();
}

/// Mark the OS crontab sync state unhealthy after a failed sync attempt.
#[cfg(any(not(target_os = "macos"), test))]
pub(crate) fn record_crontab_sync_failure(err: &SyncError) {
    *crontab_sync_state().lock_recover() = CrontabSyncStatus {
        ok: false,
        last_error: Some(err.to_string()),
        last_error_at: Some(now_secs()),
    };
}

#[cfg(test)]
pub(crate) fn reset_crontab_sync_status_for_tests() {
    record_crontab_sync_success();
}

/// Environment override for the `crontab -` install timeout, in seconds.
const CRONTAB_WRITE_TIMEOUT_ENV: &str = "MOADIM_CRONTAB_WRITE_TIMEOUT_SECS";

/// Default wall-clock time allowed for `crontab -` to install the generated block.
const DEFAULT_CRONTAB_WRITE_TIMEOUT: Duration = Duration::from_secs(15);

/// Poll interval used while waiting for `crontab -` to exit.
const CRONTAB_WAIT_POLL_INTERVAL: Duration = Duration::from_millis(100);

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
include!("read_crontab.rs");
