//! Persisted routine types, derived API response, and request bodies.

use croner::Cron;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use super::command::slugify;
use crate::paths::{agent_toml_path, routine_toml_path};

/// A git repository made available to a routine's agent as prompt context (not cloned by moadim).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, utoipa::ToSchema)]
pub struct Repository {
    /// Git remote URL.
    pub repository: String,
    /// Branch to use, or `None` for the remote default branch.
    #[serde(default)]
    pub branch: Option<String>,
}

/// A persisted routine: a scheduled AI-agent task.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, utoipa::ToSchema)]
pub struct Routine {
    /// Unique identifier (UUID v4).
    pub id: String,
    /// Cron expression defining when the routine runs, evaluated in the host's local
    /// system timezone (the OS crontab timezone), not UTC.
    pub schedule: String,
    /// Human name; slugified to name the workbench and tmux session.
    pub title: String,
    /// Agent registry key (e.g. `"claude"`) resolved from `~/.config/moadim/agents/`.
    pub agent: String,
    /// The task prompt handed to the agent.
    pub prompt: String,
    /// Repositories listed in the prompt as context.
    #[serde(default)]
    pub repositories: Vec<Repository>,
    /// Whether the routine is active.
    pub enabled: bool,
    /// `"managed"` for routines owned by this server.
    pub source: String,
    /// Unix timestamp (seconds) when the routine was created.
    pub created_at: u64,
    /// Unix timestamp (seconds) when the routine was last updated.
    pub updated_at: u64,
    /// Unix timestamp (seconds) when the routine was last manually triggered, if ever.
    pub last_triggered_at: Option<u64>,
    /// How long (seconds) a finished run's workbench is retained before auto-cleanup removes it.
    /// `None` falls back to [`DEFAULT_TTL_SECS`]. Sessions still running are never reaped.
    #[serde(default)]
    pub ttl_secs: Option<u64>,
}

/// Default retention for a finished run's workbench when a routine sets no explicit `ttl_secs`.
pub const DEFAULT_TTL_SECS: u64 = 7 * 24 * 60 * 60;

impl Routine {
    /// Retention for this routine's finished workbenches: its `ttl_secs` or [`DEFAULT_TTL_SECS`].
    pub fn effective_ttl_secs(&self) -> u64 {
        self.ttl_secs.unwrap_or(DEFAULT_TTL_SECS)
    }
}

/// A [`Routine`] enriched with derived, non-persisted fields for API responses.
#[derive(Debug, Clone, Serialize, JsonSchema, utoipa::ToSchema)]
pub struct RoutineResponse {
    /// The underlying routine.
    #[serde(flatten)]
    pub routine: Routine,
    /// `true` if an agent config exists at `~/.config/moadim/agents/<agent>.toml`.
    pub agent_registered: bool,
    /// Absolute path to the routine's `routine.toml` file on disk.
    pub file_path: String,
    /// Human-readable description of the schedule, including the timezone the
    /// cron expression is interpreted in, or `null` if it cannot be parsed.
    pub schedule_description: Option<String>,
    /// IANA name of the local timezone the schedule is interpreted in (e.g.
    /// `"Asia/Jerusalem"`), or `null` if it cannot be determined. Cron
    /// expressions are evaluated in this timezone, **not** UTC.
    pub timezone: Option<String>,
}

/// The IANA name of the host's local timezone (e.g. `"Asia/Jerusalem"`).
///
/// Managed schedules run via the local `crontab`, which interprets cron
/// expressions in this timezone — not UTC. Returns `None` if it can't be
/// determined.
pub fn local_timezone() -> Option<String> {
    iana_time_zone::get_timezone().ok()
}

impl RoutineResponse {
    /// Build a response from `routine`, deriving registration status and schedule description.
    pub fn from_routine(routine: Routine) -> Self {
        let agent_registered = agent_toml_path(&routine.agent).exists();
        let file_path = routine_toml_path(&slugify(&routine.title))
            .to_string_lossy()
            .into_owned();
        let timezone = local_timezone();
        let schedule_description = routine.schedule.parse::<Cron>().ok().map(|c| {
            let desc = c.describe();
            match &timezone {
                Some(tz) => format!("{desc} ({tz})"),
                None => desc,
            }
        });
        Self {
            routine,
            agent_registered,
            file_path,
            schedule_description,
            timezone,
        }
    }
}

/// Result of an on-demand workbench cleanup sweep.
#[derive(Debug, Clone, Serialize, JsonSchema, utoipa::ToSchema)]
pub struct CleanupResponse {
    /// Number of finished, expired run workbenches removed by this sweep.
    pub removed: usize,
}

/// Thread-safe shared store of routines keyed by ID.
pub type RoutineStore = Arc<Mutex<HashMap<String, Routine>>>;

/// Create an empty [`RoutineStore`].
#[cfg(test)]
pub fn new_store() -> RoutineStore {
    Arc::new(Mutex::new(HashMap::new()))
}

/// Serde default for boolean fields that should default to `true`.
pub(crate) fn bool_true() -> bool {
    true
}

/// Request body for creating a new routine.
#[derive(Deserialize, JsonSchema, utoipa::ToSchema)]
pub struct CreateRoutineRequest {
    /// Cron expression for the new routine. Evaluated in the host's local system
    /// timezone (the OS crontab timezone), not UTC.
    pub schedule: String,
    /// Human name for the routine.
    pub title: String,
    /// Agent registry key to launch.
    pub agent: String,
    /// Task prompt.
    pub prompt: String,
    /// Repositories to list as context (defaults to empty).
    #[serde(default)]
    pub repositories: Vec<Repository>,
    /// Whether to create the routine enabled (defaults to `true`).
    #[serde(default = "bool_true")]
    pub enabled: bool,
    /// Workbench retention in seconds for finished runs; `None` uses [`DEFAULT_TTL_SECS`].
    #[serde(default)]
    pub ttl_secs: Option<u64>,
}

/// Request body for partially updating an existing routine.
#[derive(Deserialize, JsonSchema, utoipa::ToSchema)]
pub struct UpdateRoutineRequest {
    /// New cron expression, or `None` to keep the existing value. Evaluated in the
    /// host's local system timezone (the OS crontab timezone), not UTC.
    pub schedule: Option<String>,
    /// New title, or `None` to keep the existing value.
    pub title: Option<String>,
    /// New agent key, or `None` to keep the existing value.
    pub agent: Option<String>,
    /// New prompt, or `None` to keep the existing value.
    pub prompt: Option<String>,
    /// New repositories list, or `None` to keep the existing value.
    pub repositories: Option<Vec<Repository>>,
    /// New enabled state, or `None` to keep the existing value.
    pub enabled: Option<bool>,
    /// New workbench TTL (seconds), or `None` to keep the existing value.
    pub ttl_secs: Option<u64>,
}
