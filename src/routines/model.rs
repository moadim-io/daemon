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
    /// Cron expression defining when the routine runs. Times are interpreted in
    /// the host's local system timezone, not UTC.
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
    /// Human-readable description of the schedule, or `null` if it cannot be parsed.
    pub schedule_description: Option<String>,
}

impl RoutineResponse {
    /// Build a response from `routine`, deriving registration status and schedule description.
    pub fn from_routine(routine: Routine) -> Self {
        let agent_registered = agent_toml_path(&routine.agent).exists();
        let file_path = routine_toml_path(&slugify(&routine.title))
            .to_string_lossy()
            .into_owned();
        let schedule_description = routine.schedule.parse::<Cron>().ok().map(|c| c.describe());
        Self {
            routine,
            agent_registered,
            file_path,
            schedule_description,
        }
    }
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
    /// Cron expression for the new routine. Times are interpreted in the host's
    /// local system timezone, not UTC.
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
}

/// Request body for partially updating an existing routine.
#[derive(Deserialize, JsonSchema, utoipa::ToSchema)]
pub struct UpdateRoutineRequest {
    /// New cron expression, or `None` to keep the existing value. Times are
    /// interpreted in the host's local system timezone, not UTC.
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
}
