//! Run-list response models.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Outcome of a single past run, derived from its workbench on disk.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, utoipa::ToSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    /// The tmux session is still alive.
    Running,
    /// The agent process exited `0`.
    Success,
    /// The agent process exited non-zero.
    Failed,
    /// The session is gone but no exit code was recorded (killed, crashed before writing it, or from
    /// a build predating exit-code capture).
    Unknown,
}

/// One past (or in-progress) run of a routine, listed newest-first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema, utoipa::ToSchema)]
pub struct RunSummary {
    /// Workbench directory name (`{slug}-{unix_secs}`); pass to `GET /routines/{id}/runs/{workbench}/log`.
    pub workbench: String,
    /// Unix seconds the run was triggered.
    pub started_at: u64,
    /// `started_at` as a human-readable local (daemon machine's timezone) timestamp.
    pub started_at_local: String,
    /// Unix seconds the run finished (`exit_code` file's mtime), `None` while running or unknown.
    pub finished_at: Option<u64>,
    /// `finished_at` as a human-readable local timestamp, when finished.
    pub finished_at_local: Option<String>,
    /// Success/failure/running/unknown from exit code and tmux liveness.
    pub status: RunStatus,
    /// Process exit code, when recorded.
    pub exit_code: Option<i32>,
    /// Unix seconds this run's workbench is due to be reaped.
    pub retention_expires_at: Option<u64>,
}

/// One past (or in-progress) run, across every routine, listed newest-first — the fleet-wide
/// counterpart to [`RunSummary`] backing an overview "recent runs" view.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema, utoipa::ToSchema)]
pub struct FleetRunSummary {
    /// The routine this run belongs to.
    pub routine_id: String,
    /// The routine's current title.
    pub routine_title: String,
    /// Workbench directory name (`{slug}-{unix_secs}`).
    pub workbench: String,
    /// Unix seconds the run was triggered.
    pub started_at: u64,
    /// `started_at` as a human-readable local (daemon machine's timezone) timestamp.
    pub started_at_local: String,
    /// Unix seconds the run finished (`exit_code` file's mtime), `None` while running or unknown.
    pub finished_at: Option<u64>,
    /// `finished_at` as a human-readable local timestamp, when finished.
    pub finished_at_local: Option<String>,
    /// Success/failure/running/unknown from exit code and tmux liveness.
    pub status: RunStatus,
    /// Process exit code, when recorded.
    pub exit_code: Option<i32>,
}
