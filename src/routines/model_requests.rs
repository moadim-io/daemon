//! `create_routine` / `update_routine` request bodies, split out of `model.rs` to keep that file
//! under the line-count gate.

use schemars::JsonSchema;
use serde::Deserialize;

use super::{bool_true, Repository};

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
    /// Model ID to run the agent with, or `None` to use the agent's own default. A
    /// blank/whitespace-only value is treated the same as `None`.
    #[serde(default)]
    pub model: Option<String>,
    /// Task prompt.
    pub prompt: String,
    /// A very short (at most 5 lines) statement of the routine's goal. Optional; `None` leaves it
    /// unset. When present it is rendered into the agent's `prompt.md` as a `## Goal` preamble.
    #[serde(default)]
    pub goal: Option<String>,
    /// Repositories to list as context (defaults to empty).
    #[serde(default)]
    pub repositories: Vec<Repository>,
    /// Machines to run this routine on (defaults to empty = runs nowhere until assigned).
    #[serde(default)]
    pub machines: Vec<String>,
    /// Whether to create the routine enabled (defaults to `true`).
    #[serde(default = "bool_true")]
    pub enabled: bool,
    /// Workbench retention in seconds for finished runs; caps the cron-derived
    /// retention lower. `None` uses `min(MAX_TTL_SECS, cron interval)`. Must be
    /// greater than zero when set; `0` is rejected (#233).
    #[serde(default)]
    pub ttl_secs: Option<u64>,
    /// Max wall-clock seconds a run may execute before the watchdog kills its hung
    /// session. `None` uses the default cap (`MAX_RUNTIME_SECS`). Must be greater
    /// than zero when set; `0` is rejected (#233).
    #[serde(default)]
    pub max_runtime_secs: Option<u64>,
    /// Free-form labels for the routine (defaults to empty). Each entry is trimmed
    /// and must be non-blank.
    #[serde(default)]
    pub tags: Vec<String>,
    /// Non-secret environment variables to inject into the agent's shell session at launch,
    /// written to `routine.toml`'s `[env]` table (defaults to empty). Keys must match
    /// `[A-Za-z_][A-Za-z0-9_]*`; values must not contain newlines (#408). For secrets, edit the
    /// gitignored `routine.local.toml` sidecar directly on disk instead — it is never accepted
    /// over the API.
    #[serde(default)]
    pub env: std::collections::HashMap<String, String>,
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
    /// New model ID, or `None` to keep the existing value. A blank/whitespace-only value clears
    /// the model back to the agent's own default.
    pub model: Option<String>,
    /// New prompt, or `None` to keep the existing value.
    pub prompt: Option<String>,
    /// New goal, or `None` to keep the existing value. Send an empty string to clear it.
    pub goal: Option<String>,
    /// New repositories list, or `None` to keep the existing value.
    pub repositories: Option<Vec<Repository>>,
    /// New machines targeting list, or `None` to keep the existing value.
    pub machines: Option<Vec<String>>,
    /// New enabled state, or `None` to keep the existing value.
    pub enabled: Option<bool>,
    /// New workbench TTL (seconds), or `None` to keep the existing value. Must be
    /// greater than zero when set; `0` is rejected (#233).
    pub ttl_secs: Option<u64>,
    /// New max runtime (seconds) for a single run, or `None` to keep the existing value. Must be
    /// greater than zero when set; `0` is rejected (#233).
    pub max_runtime_secs: Option<u64>,
    /// New tags list, or `None` to keep the existing value.
    pub tags: Option<Vec<String>>,
    /// New tracked `[env]` map, or `None` to keep the existing value. Replaces the whole map (not
    /// merged); send the full desired set. See [`CreateRoutineRequest::env`] for validation rules
    /// and the `routine.local.toml` secrets sidecar.
    pub env: Option<std::collections::HashMap<String, String>>,
}
