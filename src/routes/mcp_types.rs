//! Input structs for the MCP tools defined in `mcp.rs`, split out to keep that file under the
//! repo's 500-line-per-file gate.

use schemars::JsonSchema;
use serde::Deserialize;

/// Input for tools that operate on a single routine by ID.
#[derive(Deserialize, JsonSchema)]
pub(super) struct IdInput {
    /// UUID of the target routine.
    pub(super) id: String,
}

/// Input for the `list_routines` MCP tool.
#[derive(Deserialize, JsonSchema)]
pub(super) struct ListRoutinesParam {
    /// When `true` (the default), only return routines targeting the current machine.
    /// Pass `false` to see routines from all machines.
    pub(super) local_only: Option<bool>,
    /// When `true`, include each routine's `prompt` in the response. Defaults to `false` so listings stay compact; use `get_routine` to see a single routine's prompt.
    pub(super) include_prompts: Option<bool>,
}

/// Input for the `lock_routines` MCP tool.
#[derive(Deserialize, JsonSchema)]
pub(super) struct LockRoutinesInput {
    /// Which sentinel to create: `"shared"` (committed `.lock`) or `"local"` (gitignored `.local.lock`).
    pub(super) scope: String,
}

/// Input for the `unlock_routines` MCP tool.
#[derive(Deserialize, JsonSchema)]
pub(super) struct UnlockRoutinesInput {
    /// Which sentinel(s) to remove: `"shared"`, `"local"`, or `"all"` (both).
    pub(super) scope: String,
}

/// Input for the `create_flag` MCP tool.
#[derive(Deserialize, JsonSchema)]
pub(super) struct CreateFlagInput {
    /// UUID of the routine to flag.
    pub(super) id: String,
    /// Free-text flag category. Common examples: "bug", "gap", `edge_case`, "question", "blocker"
    /// — any string is accepted.
    pub(super) r#type: String,
    /// Free-text description of what's unclear.
    pub(super) description: String,
    /// `"general"` (committed, shared via git) or `"local"` (gitignored, machine-local).
    pub(super) scope: String,
}

/// Input for the `resolve_flag` MCP tool.
#[derive(Deserialize, JsonSchema)]
pub(super) struct ResolveFlagInput {
    /// UUID of the flagged routine.
    pub(super) id: String,
    /// Flag filename, as returned by `create_flag`/`list_flags`.
    pub(super) filename: String,
}

/// Input for the `snooze_routine` MCP tool.
#[derive(Deserialize, JsonSchema)]
pub(super) struct SnoozeRoutineInput {
    /// UUID of the routine to snooze.
    pub(super) id: String,
    /// Unix timestamp (seconds) to skip scheduled fires until, or omit/null. Mutually exclusive
    /// with `skip_runs`.
    pub(super) snoozed_until: Option<u64>,
    /// Number of upcoming scheduled fires to skip, or omit/null. Mutually exclusive with
    /// `snoozed_until`.
    pub(super) skip_runs: Option<u32>,
}

/// Input for the `set_power_saving` MCP tool.
#[derive(Deserialize, JsonSchema)]
pub(super) struct SetPowerSavingInput {
    /// UUID of the routine to update.
    pub(super) id: String,
    /// `true` to pause scheduled and manual firing for power saving, `false` to resume.
    pub(super) active: bool,
}

/// Input for the `update_routine` MCP tool.
#[derive(Deserialize, JsonSchema)]
pub(super) struct UpdateRoutineInput {
    /// UUID of the routine to update.
    pub(super) id: String,
    /// New cron expression, or `None` to keep the existing value. Evaluated in the
    /// host's local system timezone (the OS crontab timezone), not UTC.
    pub(super) schedule: Option<String>,
    /// New title, or `None` to keep the existing value.
    pub(super) title: Option<String>,
    /// New agent key, or `None` to keep the existing value.
    pub(super) agent: Option<String>,
    /// New model ID, or `None` to keep the existing value. A blank/whitespace-only value clears
    /// the model back to the agent's own default.
    pub(super) model: Option<String>,
    /// New prompt, or `None` to keep the existing value.
    pub(super) prompt: Option<String>,
    /// New goal (a very short, ≤5-line statement of the routine's purpose), or `None` to keep the
    /// existing value. Send an empty string to clear it.
    pub(super) goal: Option<String>,
    /// New repositories list, or `None` to keep the existing value.
    pub(super) repositories: Option<Vec<crate::routines::Repository>>,
    /// New auto-pull setting, or `None` to keep the existing value.
    pub(super) auto_pull: Option<bool>,
    /// New machines targeting list, or `None` to keep the existing value.
    pub(super) machines: Option<Vec<String>>,
    /// New enabled state, or `None` to keep the existing value.
    pub(super) enabled: Option<bool>,
    /// New workbench TTL (seconds) for finished runs, or `None` to keep the existing value. Must
    /// be greater than zero when set; `0` is rejected (#233).
    pub(super) ttl_secs: Option<u64>,
    /// New max runtime (seconds) for a single run before the watchdog kills it, or `None` to keep
    /// the existing value. Must be greater than zero when set; `0` is rejected (#233).
    pub(super) max_runtime_secs: Option<u64>,
    /// New tags list, or `None` to keep the existing value.
    pub(super) tags: Option<Vec<String>>,
}
