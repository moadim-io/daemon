#![allow(
    clippy::wildcard_imports,
    clippy::too_many_arguments,
    clippy::needless_pass_by_value,
    dead_code,
    reason = "split-out module preserves existing code while keeping files under the linecheck limit"
)]
use super::*;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A persisted routine: a scheduled AI-agent task.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, utoipa::ToSchema)]
pub struct Routine {
    /// Unique identifier (UUID v4).
    pub id: String,
    /// Primary cron expression defining when the routine runs, evaluated in the host's local
    /// system timezone (the OS crontab timezone), not UTC. Kept for backward-compatible clients.
    pub schedule: String,
    /// All cron expressions defining when the routine runs. The first entry mirrors [`Routine::schedule`].
    #[serde(default)]
    pub schedules: Vec<String>,
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
    /// Optional user-provided reason captured when the routine was manually disabled.
    ///
    /// Persisted in the tracked `disabled.json` marker, not `routine.toml`. `None` for enabled
    /// routines, disabled routines whose marker predates reason metadata, and malformed markers
    /// whose presence still disables the routine.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disabled_reason: Option<String>,
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
    /// Whether this routine is allowed to run while the host is in system power saving.
    ///
    /// This is user-owned routine metadata, persisted in `routine.toml`: critical maintenance or
    /// alerting routines can opt out of the host-level battery/Low Power Mode throttle while normal
    /// routines keep the conservative default and skip launches until the host leaves power saving.
    #[serde(default)]
    pub power_saving_exempt: bool,
    /// Number of scheduled/manual runs that finished failed-or-unknown in a row, most recently
    /// first — reset to `0` the instant any run succeeds. Daemon-owned runtime state: persisted in
    /// the gitignored `state.local.toml` sidecar, not `routine.toml`, mirroring
    /// [`Routine::power_saving`]. Counted by `crate::routines::cleanup::circuit_breaker` as each
    /// run's outcome becomes durable; see [`Routine::failure_threshold`] and issue #521.
    #[serde(default)]
    pub consecutive_failures: u32,
    /// Human-readable reason this routine was auto-disabled by the failure circuit-breaker, or
    /// `None` if it has never tripped one (or a user has since manually re-enabled it, which clears
    /// this — see `svc_update`). Distinguishes an auto-disable from a user-initiated one: both flip
    /// [`Routine::enabled`] to `false`, but only the former sets a reason. Daemon-owned runtime
    /// state, persisted in `state.local.toml` alongside [`Routine::consecutive_failures`].
    #[serde(default)]
    pub auto_disabled_reason: Option<String>,
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
    /// Consecutive failed-or-unknown-outcome runs after which this routine auto-disables — the
    /// opt-in failure circuit-breaker (issue #521). `None` or `0` opts out, preserving today's
    /// behavior of retrying forever no matter how many times in a row a routine has failed; this is
    /// the default so existing routines are unaffected. A "failed" run here is any [`RunStatus`]
    /// other than `Success`, including `Unknown` (session gone with no exit code — e.g. force-killed
    /// by the max-runtime watchdog): a routine that only ever hangs and gets killed is exactly the
    /// resource-wasting loop this breaker exists to stop. Tracked config, written to `routine.toml`
    /// like [`Routine::ttl_secs`]/[`Routine::max_runtime_secs`]; unlike those two, `0` is a valid,
    /// meaningful value here (opt-out) rather than a rejected one. See
    /// `crate::routines::cleanup::circuit_breaker` for where it's enforced.
    #[serde(default)]
    pub failure_threshold: Option<u32>,
    /// Optional per-routine failure notification hooks. Empty means use the global hooks, if any.
    #[serde(default)]
    pub notifications: FailureNotificationConfig,
    /// Free-form labels for grouping and filtering routines (e.g. `"triage"`, `"nightly"`).
    /// Defaults to empty; each entry is trimmed and must be non-blank.
    #[serde(default)]
    pub tags: Vec<String>,
    /// Non-secret environment variables injected into the agent's shell session at launch,
    /// tracked in `routine.toml`'s `[env]` table (see [`crate::routines::command::build_routine_command`]).
    ///
    /// **Never serialized to JSON** (`skip_serializing`): this field can hold values a routine
    /// author committed to the tracked `routine.toml`, and a gitignored `routine.local.toml`
    /// sidecar can layer secret overrides on top at launch time (never held here at all — see
    /// [`crate::routine_storage::read_local_env`]). Neither belongs in an API response, the UI, or
    /// a log line: [`RoutineResponse::env_keys`] surfaces the *names* only, so a client can show
    /// what's set without ever seeing a value. Keys must match `[A-Za-z_][A-Za-z0-9_]*` and
    /// values must not contain newlines (enforced at create/update time — see
    /// `service_validate::validate_env`).
    #[serde(default, skip_serializing)]
    pub env: std::collections::HashMap<String, String>,
    /// IANA timezone name (e.g. `"Asia/Jerusalem"`) this routine's schedule is interpreted in,
    /// overriding the host's local system timezone. `None` (the default) preserves today's
    /// behavior: the schedule runs in whatever zone the host crontab itself uses, which silently
    /// changes if the host's zone ever changes (issue #405).
    ///
    /// Emitted as a `CRON_TZ=<tz>` directive ahead of this routine's line(s) in the managed
    /// crontab block ([`crate::sync::routines`]). Only vixie-cron/cronie (Linux) honor `CRON_TZ`;
    /// BSD `cron` (macOS) does not, so setting this field is rejected outright on non-Linux hosts
    /// (see `service_validate::validate_timezone`) rather than silently doing nothing.
    #[serde(default)]
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

/// Render a human-readable schedule description for `schedule`, appending the
/// timezone in parentheses when known. Returns `None` when the cron expression
/// cannot be parsed.
pub(crate) fn describe_schedule(schedule: &str, timezone: Option<&str>) -> Option<String> {
    schedule.parse::<Cron>().ok().map(|cron| {
        let desc = cron.describe();
        match timezone {
            Some(tz) => format!("{desc} ({tz})"),
            None => desc,
        }
    })
}
