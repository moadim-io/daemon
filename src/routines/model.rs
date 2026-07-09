//! Persisted routine types, derived API response, and request bodies.

use chrono::Local;
use croner::Cron;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use super::agents::load_agent_command;
use super::cleanup::tmux_session_prefix_alive;
use super::command::{agent_command_available, slugify, tmux_session_prefix};
use super::flags::list_flags;
use crate::paths::routine_toml_path;

/// A git repository made available to a routine's agent as prompt context (not cloned by moadim).
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
    /// Model ID to run the agent with (e.g. `"claude-sonnet-4-6"`), passed as `--model` on the
    /// agent invocation. `None` uses the agent's own default.
    #[serde(default)]
    pub model: Option<String>,
    /// The task prompt handed to the agent.
    ///
    /// Omitted from serialized output when empty. A persisted routine always has a
    /// non-blank prompt (enforced by `validate_prompt`), so this never affects
    /// `routine.toml` persistence; it lets list responses drop the prompt by blanking
    /// it in-memory (see [`RoutineListQuery::include_prompts`] / `svc_list`).
    #[serde(skip_serializing_if = "String::is_empty")]
    pub prompt: String,
    /// A very short (at most 5 lines) statement of the routine's goal — the "why" behind the
    /// prompt. Rendered into the agent's `prompt.md` as a `## Goal` preamble. `None` when unset.
    #[serde(default)]
    pub goal: Option<String>,
    /// Repositories listed in the prompt as context.
    #[serde(default)]
    pub repositories: Vec<Repository>,
    /// Machines this routine runs on. Each daemon schedules a routine only when this list names its
    /// own machine identity ([`crate::machine::current_machine`]); an **empty list runs nowhere**, so
    /// a routine is dormant until explicitly assigned. Lets one shared config repo drive different
    /// routines on different machines.
    #[serde(default)]
    pub machines: Vec<String>,
    /// Whether the routine is active.
    pub enabled: bool,
    /// `"managed"` for routines owned by this server.
    pub source: String,
    /// Unix timestamp (seconds) when the routine was created.
    pub created_at: u64,
    /// Unix timestamp (seconds) when the routine was last updated.
    pub updated_at: u64,
    /// Unix timestamp (seconds) when the routine was last manually triggered, if ever.
    ///
    /// Only manual triggers (`trigger_routine`) update this; scheduled cron firings run the built
    /// command directly and do not. Accepts the legacy `last_triggered_at` key on deserialize.
    #[serde(alias = "last_triggered_at")]
    pub last_manual_trigger_at: Option<u64>,
    /// Unix timestamp (seconds) when the routine was last fired by its cron schedule, if ever.
    ///
    /// The mirror of [`Routine::last_manual_trigger_at`] for scheduled runs: a manual trigger
    /// updates only the manual field, a scheduled firing updates only this one. The host OS crontab
    /// line runs `moadim schedule trigger <id>`, and the launch command the daemon spawns appends
    /// the Unix timestamp to the gitignored `scheduled.log` at fire time; the daemon reads the last
    /// line back on load. The daemon never writes this field directly (it is absent from
    /// `routine.toml` and the daemon-owned `state.local.toml`), so re-persisting a routine can't
    /// clobber the log.
    #[serde(default)]
    pub last_scheduled_trigger_at: Option<u64>,
    /// Unix timestamp (seconds) until which scheduled (cron) fires are skipped, or `None`.
    ///
    /// Cleared automatically the first time a scheduled fire observes `now >= snoozed_until`, which
    /// also runs that fire. Manual triggers ([`crate::routines::svc_trigger`]) ignore this entirely.
    /// Set via the `snooze_routine` MCP tool; mutually exclusive with `skip_runs`.
    #[serde(default)]
    pub snoozed_until: Option<u64>,
    /// Number of upcoming scheduled fires still to skip, or `None`.
    ///
    /// Decremented (and cleared once it reaches zero) on each skipped scheduled fire; manual
    /// triggers do not consume it. Mutually exclusive with `snoozed_until`.
    #[serde(default)]
    pub skip_runs: Option<u32>,
    /// Whether scheduled and manual firing is paused to conserve resources, independent of
    /// [`Routine::enabled`].
    ///
    /// `enabled` is user-owned intent ("I want this routine on/off"); `power_saving` is a
    /// system/policy throttle layered on top — both must hold for a firing to launch an agent
    /// (`enabled && !power_saving`). Never mutated by `svc_create`/`svc_update` (set via
    /// [`crate::routines::svc_set_power_saving`] instead), so it survives a config edit the same
    /// way `snoozed_until` and `skip_runs` do. Daemon-owned runtime state: persisted in the
    /// gitignored `state.local.toml` sidecar, not the version-controlled `routine.toml`.
    #[serde(default)]
    pub power_saving: bool,
    /// How long (seconds) a finished run's workbench is retained before auto-cleanup removes it.
    /// Caps the cron-derived retention (`min(MAX_TTL_SECS, cron interval)`) lower; it can only
    /// shorten, never extend it. `None` uses the cron-derived value. Sessions still running are
    /// never reaped. The cap and [`Routine::effective_ttl_secs`] live in the cleanup module. Must
    /// be greater than zero when set; `0` is rejected by `svc_create`/`svc_update` (#233).
    #[serde(default)]
    pub ttl_secs: Option<u64>,
    /// Maximum wall-clock seconds a single run may execute before the cleanup watchdog force-kills
    /// its (hung) tmux session, after which the workbench is reaped under the normal TTL rules.
    /// `None` uses `min(MAX_RUNTIME_SECS, cron interval)`; an explicit value can only lower that. A
    /// session still within this bound is never touched. The cap and
    /// [`Routine::effective_max_runtime_secs`] live in the cleanup module. Must be greater than
    /// zero when set; `0` is rejected by `svc_create`/`svc_update` (#233).
    #[serde(default)]
    pub max_runtime_secs: Option<u64>,
    /// Free-form labels for grouping and filtering routines (e.g. `"triage"`, `"nightly"`).
    /// Defaults to empty; each entry is trimmed and must be non-blank.
    #[serde(default)]
    pub tags: Vec<String>,
}

/// A [`Routine`] enriched with derived, non-persisted fields for API responses.
#[derive(Debug, Clone, Serialize, JsonSchema, utoipa::ToSchema)]
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
    /// Absolute path to the routine's `routine.toml` file on disk.
    pub file_path: String,
    /// Human-readable description of the schedule, including the timezone the
    /// cron expression is interpreted in, or `null` if it cannot be parsed.
    pub schedule_description: Option<String>,
    /// IANA name of the local timezone the schedule is interpreted in (e.g.
    /// `"Asia/Jerusalem"`), or `null` if it cannot be determined. Cron
    /// expressions are evaluated in this timezone, **not** UTC.
    pub timezone: Option<String>,
    /// Number of open flags raised against this routine (see [`super::flags`]). Surfaced here so
    /// listings can badge it without a separate `list_flags` round-trip per routine.
    pub flag_count: usize,
    /// Unix epoch seconds of this routine's next scheduled fire, in the host's local timezone
    /// (matching crontab semantics) — the future counterpart to `last_scheduled_trigger_at`.
    /// `None` when disabled, globally locked, or `schedule` is unparseable or has no upcoming
    /// fire (e.g. `@reboot`). See issue #369.
    pub next_run_at: Option<u64>,
    /// `true` if any fire of this routine currently has a live tmux session — i.e. an agent is
    /// running right now. Derived by probing for a session under the routine's
    /// `moadim-{slug}-` prefix (the same overlap-guard check `svc_trigger` uses, #514), not
    /// persisted. `false` whenever no `tmux` binary is available, mirroring the probe's existing
    /// best-effort "no tmux, nothing running" stance. See issue #438.
    pub is_running: bool,
}

/// The IANA name of the host's local timezone (e.g. `"Asia/Jerusalem"`).
///
/// Managed schedules run via the local `crontab`, which interprets cron
/// expressions in this timezone — not UTC. Returns `None` if it can't be
/// determined.
pub fn local_timezone() -> Option<String> {
    iana_time_zone::get_timezone().ok()
}

/// Render a human-readable schedule description for `schedule`, appending the
/// timezone in parentheses when known. Returns `None` when the cron expression
/// cannot be parsed.
fn describe_schedule(schedule: &str, timezone: Option<&str>) -> Option<String> {
    schedule.parse::<Cron>().ok().map(|cron| {
        let desc = cron.describe();
        match timezone {
            Some(tz) => format!("{desc} ({tz})"),
            None => desc,
        }
    })
}

/// Unix epoch seconds of `schedule`'s next fire after now, in the host's local timezone (matching
/// crontab semantics) — reusing the same `croner` evaluation as the `.ics` feed
/// ([`super::ical::build_ical`]) and the TTL sweep (`cleanup::ttl::cron_interval_secs`).
///
/// `None` when `enabled` is `false`, the daemon is globally locked (see [`crate::global_lock`]),
/// `schedule` cannot be parsed (e.g. `@reboot`), or it has no upcoming fire.
fn next_run_at(schedule: &str, enabled: bool) -> Option<u64> {
    if !enabled || crate::global_lock::is_globally_locked() {
        return None;
    }
    let cron: Cron = schedule.parse().ok()?;
    let next = cron.iter_after(Local::now()).next()?;
    u64::try_from(next.timestamp()).ok()
}

impl RoutineResponse {
    /// Build a response from `routine`, deriving registration status and schedule description.
    pub fn from_routine(routine: Routine) -> Self {
        let slug = slugify(&routine.title);
        // An agent counts as registered only if its config both exists *and* parses: a
        // present-but-malformed config is silently dropped at crontab-sync time, so reporting it as
        // registered would paint a never-firing routine as healthy. See issue #301.
        let agent_command = load_agent_command(&routine.agent);
        let agent_registered = agent_command.is_ok();
        let agent_command_available =
            agent_command.is_ok_and(|agent| agent_command_available(&agent.command));
        let file_path = routine_toml_path(&slug).to_string_lossy().into_owned();
        let timezone = local_timezone();
        let schedule_description = describe_schedule(&routine.schedule, timezone.as_deref());
        let flag_count = list_flags(&slug).len();
        let next_run_at = next_run_at(&routine.schedule, routine.enabled);
        let is_running = tmux_session_prefix_alive(&tmux_session_prefix(&slug));
        Self {
            routine,
            agent_registered,
            agent_command_available,
            file_path,
            schedule_description,
            timezone,
            flag_count,
            next_run_at,
            is_running,
        }
    }
}

/// Result of an on-demand workbench cleanup sweep.
#[derive(Debug, Clone, Serialize, JsonSchema, utoipa::ToSchema)]
pub struct CleanupResponse {
    /// Number of finished, expired run workbenches removed by this sweep.
    pub removed: usize,
    /// Total disk space reclaimed, in bytes, summed across the removed workbench trees. Additive
    /// field: existing `{"removed": N}` consumers are unaffected.
    pub freed_bytes: u64,
}

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
    /// The session is gone but no exit code was recorded (killed, crashed before
    /// writing it, or from a build predating exit-code capture).
    Unknown,
}

/// One past (or in-progress) run of a routine, listed newest-first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema, utoipa::ToSchema)]
pub struct RunSummary {
    /// Workbench directory name (`{slug}-{unix_secs}`); pass to `GET /routines/{id}/runs/{workbench}/log`.
    pub workbench: String,
    /// Unix seconds the run was triggered.
    pub started_at: u64,
    /// Unix seconds the run finished (`exit_code` file's mtime), `None` while running or unknown.
    pub finished_at: Option<u64>,
    /// Success/failure/running/unknown, derived from the exit-code file and tmux session liveness.
    pub status: RunStatus,
    /// Process exit code, when recorded.
    pub exit_code: Option<i32>,
    /// Unix seconds this run's workbench is due to be reaped (`finished_at` +
    /// [`Routine::effective_ttl_secs`]). `None` while the run hasn't finished, or once its
    /// workbench is already gone (a run restored from `runs.log` after TTL reaping).
    pub retention_expires_at: Option<u64>,
}

/// One past (or in-progress) run, across every routine, listed newest-first — the fleet-wide
/// counterpart to [`RunSummary`] backing an overview "recent runs" view.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema, utoipa::ToSchema)]
pub struct FleetRunSummary {
    /// The routine this run belongs to.
    pub routine_id: String,
    /// The routine's title, at the time of this call (not snapshotted per-run).
    pub routine_title: String,
    /// Workbench directory name (`{slug}-{unix_secs}`).
    pub workbench: String,
    /// Unix seconds the run was triggered.
    pub started_at: u64,
    /// Unix seconds the run finished (`exit_code` file's mtime), `None` while running or unknown.
    pub finished_at: Option<u64>,
    /// Success/failure/running/unknown, derived from the exit-code file and tmux session liveness.
    pub status: RunStatus,
    /// Process exit code, when recorded.
    pub exit_code: Option<i32>,
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
}

#[cfg(test)]
#[path = "model_tests.rs"]
mod model_tests;
