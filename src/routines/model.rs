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

#[cfg(test)]
use super::command::slugify;
use super::command::{agent_command_available, setup_step_available, tmux_session_prefix};
use super::flags::list_flags;
use crate::paths::routines_dir;

/// A git repository the daemon pre-clones (via a persistent local mirror, see
/// [`crate::paths::repo_cache_dir`]) into the workbench before the agent launches (#466).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, utoipa::ToSchema)]
pub struct Repository {
    /// Git remote URL.
    pub repository: String,
    /// Branch to use, or `None` for the remote default branch.
    #[serde(default)]
    pub branch: Option<String>,
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
    /// `true` if an agent config exists at `~/.config/moadim/agents/<agent>.toml` *and* parses
    /// successfully. A present-but-malformed config is silently dropped at crontab-sync time, so
    /// it reports `false` here too — file existence alone is not "registered".
    pub agent_registered: bool,
    /// `true` if the agent config's `command` (e.g. `claude`, `codex`) resolves to an executable
    /// on the daemon's `PATH`. Distinct from [`Self::agent_registered`]: a routine can have a
    /// present, well-formed agent config yet reference a binary that isn't installed, in which
    /// case the cron firing launches a tmux session that dies immediately with "command not
    /// found" — a silent no-op indistinguishable from a healthy routine by `agent_registered`
    /// alone. `false` whenever the agent config is missing, unreadable, or malformed, since no
    /// `command` can be resolved in that case either.
    pub agent_command_available: bool,
    /// `true` if the agent config has no `setup` step, or that step's first whitespace-delimited
    /// token (the interpreter/binary it shells out to, e.g. `python3` for the built-in `claude`
    /// agent's workspace-trust seeding) resolves on the daemon's `PATH`. Distinct from
    /// [`Self::agent_command_available`]: the agent's own `command` can be installed while its
    /// `setup` step still shells out to something that isn't — in which case the launch command's
    /// fail-fast guard (`build_routine_command`) aborts the run before the agent ever starts, a
    /// failure otherwise invisible to anything checking only `agent_command_available`. `false`
    /// here means the run is expected to abort in `setup` rather than actually launch the agent.
    /// `false` whenever the agent config is missing, unreadable, or malformed too (mirrors
    /// `agent_command_available`'s pessimistic default) — `agent_registered` is the field that
    /// distinguishes that case. See issue #404.
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
