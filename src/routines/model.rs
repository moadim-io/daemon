//! Persisted routine types, derived API response, and request bodies.

use chrono::Local;
use croner::Cron;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use super::agents::load_agent_command;
use super::cleanup::tmux_session_prefix_alive;

#[allow(
    clippy::missing_docs_in_private_items,
    reason = "split-out module keeps the file under the linecheck limit"
)]
#[path = "local_timezone.rs"]
mod local_timezone;
pub(crate) use local_timezone::*;
#[allow(
    clippy::missing_docs_in_private_items,
    reason = "split-out module keeps the file under the linecheck limit"
)]
#[path = "next_run_at.rs"]
mod next_run_at;
pub(crate) use next_run_at::*;
#[allow(
    clippy::missing_docs_in_private_items,
    reason = "split-out module keeps the file under the linecheck limit"
)]
#[path = "missed_run_alert.rs"]
mod missed_run_alert;
pub(crate) use missed_run_alert::*;
#[path = "failure_notification_config.rs"]
mod failure_notification_config;
pub use failure_notification_config::FailureNotificationConfig;

#[cfg(test)]
use super::command::slugify;
use super::command::{agent_command_available, setup_step_available, tmux_session_prefix};
use super::flags::list_flags;
use crate::paths::routines_dir;

/// A git repository the daemon can pre-sync (via a persistent local mirror, see
/// [`crate::paths::repo_cache_dir`]) into the workbench before the agent launches (#466).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, utoipa::ToSchema)]
#[allow(
    clippy::struct_field_names,
    reason = "wire format keeps repository.repository key"
)]
pub struct Repository {
    /// Git remote URL.
    pub repository: String,
    /// Branch to use, or `None` for the remote default branch.
    #[serde(default)]
    pub branch: Option<String>,
    /// Whether to fetch/materialize before each run; `false` leaves checkout state to the routine.
    #[serde(default = "bool_true")]
    pub auto_pull: bool,
}

/// Field to sort a routine listing by.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, JsonSchema, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum RoutineSort {
    /// Creation time (default).
    #[default]
    Created,
    /// Last update time.
    Updated,
    /// Title, alphabetically (case-insensitive).
    Title,
    /// Primary (first) repository URL, alphabetically; routines with no
    /// repository sort last.
    Repository,
}

/// Sort direction for a routine listing.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, JsonSchema, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum SortOrder {
    /// Ascending (default): oldest / A→Z first.
    #[default]
    Asc,
    /// Descending: newest / Z→A first.
    Desc,
}

/// Query parameters for `GET /routines`: filter and sort a routine listing,
/// notably by the repositories a routine references.
#[derive(Debug, Clone, Default, Deserialize, JsonSchema, utoipa::IntoParams)]
#[serde(default)]
#[into_params(parameter_in = Query)]
pub struct RoutineListQuery {
    /// Keep only routines with at least one repository whose URL contains this
    /// substring (case-insensitive). Empty or absent keeps every routine.
    pub repository: Option<String>,
    /// Field to sort by (default: creation time).
    pub sort: RoutineSort,
    /// Sort direction (default: ascending).
    pub order: SortOrder,
    /// When `true`, only return routines whose `machines` list includes the current machine.
    /// Defaults to `false` (return all routines, preserving backwards compatibility).
    pub local_only: Option<bool>,
    /// When `true`, include each routine's `prompt` in the response. Defaults to `false`:
    /// the prompt (often the largest field) is omitted so listings stay compact. Fetch a
    /// single routine with `svc_get` / `GET /routines/{id}` to always see its prompt.
    pub include_prompts: Option<bool>,
}

/// Query parameters for `GET /routines.ics`: optionally scope the feed to one routine.
#[derive(Debug, Clone, Default, Deserialize, JsonSchema, utoipa::IntoParams)]
#[serde(default)]
#[into_params(parameter_in = Query)]
pub struct IcalFeedQuery {
    /// Render only the fire times of the routine with this UUID. Absent (the default)
    /// renders every enabled routine. An unknown or disabled id yields a well-formed
    /// empty calendar.
    pub routine: Option<String>,
}

/// A [`Routine`] enriched with derived, non-persisted fields for API responses.
#[derive(Debug, Clone, Serialize, JsonSchema, utoipa::ToSchema)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "agent_registered/agent_command_available/agent_setup_available/is_running are \
              independent, already-documented probes, not combinatorial state, and this is a \
              #[serde(flatten)] HTTP response DTO — collapsing them into an enum would break the \
              JSON shape existing API clients parse"
)]
pub struct RoutineResponse {
    /// The underlying routine.
    #[serde(flatten)]
    pub routine: Routine,
    /// `true` if the agent config exists and parses; malformed configs report `false` too.
    pub agent_registered: bool,
    /// `true` if the agent config's `command` resolves on the daemon `PATH`.
    pub agent_command_available: bool,
    /// `true` if the agent config has no `setup` step or its setup binary resolves on `PATH`.
    pub agent_setup_available: bool,
    /// Absolute path to the routine's `routine.toml` file on disk.
    pub file_path: String,
    /// Parent folder relative to `routines/`, derived from the routine's filesystem location.
    /// `None` means the routine lives directly under `routines/`.
    pub folder: Option<String>,
    /// Last path segment of the routine's filesystem location.
    pub slug: String,
    /// Full routine directory relative to `routines/`.
    pub rel_path: String,
    /// Human-readable description of the schedule, including the timezone the
    /// cron expression is interpreted in, or `null` if it cannot be parsed.
    pub schedule_description: Option<String>,
    /// Human-readable descriptions of every schedule.
    #[serde(default)]
    pub schedule_descriptions: Vec<String>,
    /// IANA name of the local timezone the schedule is interpreted in (e.g.
    /// `"Asia/Jerusalem"`), or `null` if it cannot be determined. Cron
    /// expressions are evaluated in this timezone, **not** UTC.
    pub timezone: Option<String>,
    /// Number of open flags raised against this routine (see [`super::flags`]). Surfaced here so
    /// listings can badge it without a separate `list_flags` round-trip per routine.
    pub flag_count: usize,
    /// Unix epoch seconds of this routine's next scheduled fire, in the host's local timezone
    /// (matching crontab semantics) — the future counterpart to `last_scheduled_trigger_at`.
    /// `None` when disabled, globally locked, or no schedule is parseable / has an upcoming
    /// fire (e.g. `@reboot`). See issue #369. For multi-schedule routines this is the earliest
    /// upcoming fire across all schedules.
    pub next_run_at: Option<u64>,
    /// Latest missed scheduled fire, if any. Alert-only: never launches catch-up runs.
    pub missed_scheduled_run_at: Option<u64>,
    /// `true` if any fire of this routine currently has a live tmux session — i.e. an agent is
    /// running right now. Derived by probing for a session under the routine's
    /// `moadim-{slug}-` prefix (the same overlap-guard check `svc_trigger` uses, #514), not
    /// persisted. `false` whenever no `tmux` binary is available, mirroring the probe's existing
    /// best-effort "no tmux, nothing running" stance. See issue #438.
    pub is_running: bool,
    /// Names (never values) of every environment variable set for this routine, merging the
    /// tracked `routine.toml` `[env]` table with the untracked `routine.local.toml` sidecar
    /// (secrets), deduplicated and sorted. Lets a client show *what* is configured without ever
    /// exposing a secret value over the API (issue #408).
    pub env_keys: Vec<String>,
}

#[path = "model_runs.rs"]
mod model_runs;
pub use model_runs::{FleetRunSummary, RunStatus, RunSummary};

#[path = "model_requests.rs"]
mod model_requests;
pub use model_requests::{CreateRoutineRequest, UpdateRoutineRequest};

#[cfg(test)]
#[path = "model_tests.rs"]
mod model_tests;
