//! Field-validation helpers shared by [`super::svc_create`] and [`super::svc_update`].

use crate::error::AppError;
use crate::routines::agents::{available_agents, load_agent_command, AgentLoadError};
use crate::routines::command::{is_valid_env_key, validate_placeholders};
use crate::routines::model::Repository;

#[allow(
    clippy::missing_docs_in_private_items,
    reason = "split-out module keeps the file under the linecheck limit"
)]
#[path = "service_validate_part2.rs"]
mod service_validate_part2;
pub(crate) use service_validate_part2::*;

/// Map a [`crate::routine_storage::write_routine`] failure to an [`AppError`], turning the
/// on-disk slug-collision guard (#188, `ErrorKind::AlreadyExists`) into a 409 the caller can act
/// on instead of a generic 500.
pub(super) fn map_write_routine_err(err: &std::io::Error) -> AppError {
    if err.kind() == std::io::ErrorKind::AlreadyExists {
        AppError::Conflict(err.to_string())
    } else {
        AppError::Internal
    }
}

/// Reject a blank (empty or whitespace-only) required text field.
///
/// An empty `prompt` makes a routine fire forever with no task (#224); an empty
/// `title` yields an empty routine-origin disclosure name and a bare `"routine"`
/// slug (#226). Both are caught here before anything is persisted.
pub(super) fn reject_blank(field: &str, value: &str) -> Result<(), AppError> {
    if value.trim().is_empty() {
        return Err(AppError::BadRequest(format!(
            "routine {field} must not be empty"
        )));
    }
    Ok(())
}

/// Reject a zero-second duration for an optional cap (`None` keeps the default).
///
/// `ttl_secs: 0` reaps a finished run's logs instantly and `max_runtime_secs: 0`
/// makes the watchdog kill the session the moment it starts (#233), so a supplied
/// value must be positive.
pub(super) fn reject_zero_secs(field: &str, value: Option<u64>) -> Result<(), AppError> {
    if value == Some(0) {
        return Err(AppError::BadRequest(format!(
            "routine {field} must be greater than zero"
        )));
    }
    Ok(())
}

/// Reject a duration cap that exceeds the cron-derived `ceiling` for the routine's schedule.
///
/// `effective_ttl_secs` / `effective_max_runtime_secs` clamp an explicit value to
/// `min(MAX_*_SECS, cron interval)`, so a larger value is silently inert — accepted, persisted, and
/// shown in the UI, yet never enforced. Rejecting it up front (naming the ceiling) keeps the stored
/// config honest, mirroring the other `reject_*` / `validate_*` boundary checks (#468).
pub(super) fn reject_over_ceiling(
    field: &str,
    value: Option<u64>,
    ceiling: u64,
) -> Result<(), AppError> {
    if let Some(secs) = value {
        if secs > ceiling {
            return Err(AppError::BadRequest(format!(
                "routine {field} {secs} exceeds the ceiling of {ceiling}s derived from this routine's schedule"
            )));
        }
    }
    Ok(())
}

/// Reject a referenced agent that is unknown or whose `<name>.toml` is present but unparseable.
///
/// Two failures are surfaced at edit time (REST 400 / MCP) instead of slipping through to fire time,
/// where they would only be logged and the routine silently skipped:
///
/// * An agent not present in the registry resolves to no command at fire time (#139). Mirrors the
///   `validate_cron` / slug-conflict guards.
/// * An agent whose config is present on disk but cannot be parsed (#189).
/// * An agent whose config parses but whose `args` carry a typo'd placeholder or no prompt
///   placeholder at all, so it would launch with a garbage or empty task (#322).
///
/// A *missing* config for a registered agent is intentionally allowed: the file may be created later,
/// and the missing-file case is handled (warned + skipped) downstream exactly as before.
pub(super) fn validate_agent(agent: &str) -> Result<(), AppError> {
    let agents = available_agents();
    if !agents.iter().any(|known| known == agent) {
        return Err(AppError::BadRequest(format!(
            "unknown agent \"{agent}\"; valid agents: {}",
            agents.join(", ")
        )));
    }
    match load_agent_command(agent) {
        Ok(command) => validate_placeholders(&command.args)
            .map_err(|reason| AppError::BadRequest(format!("agent {agent:?} config: {reason}"))),
        Err(AgentLoadError::Missing) => Ok(()),
        Err(AgentLoadError::Parse(err)) => Err(AppError::BadRequest(format!(
            "agent {agent:?} has a malformed config: {err}"
        ))),
        // An existing-but-unreadable config (e.g. permissions) would otherwise pass validation and
        // leave a green-dot routine that never fires; surface it now instead of silently dropping it.
        Err(AgentLoadError::Unreadable(err)) => Err(AppError::BadRequest(format!(
            "agent {agent:?} has an unreadable config: {err}"
        ))),
    }
}

/// Reject a prompt that is empty or whitespace-only with `400 Bad Request`.
///
/// The prompt is the one field that defines what a routine actually does. A blank
/// prompt still produces a valid `prompt.compiled.local.md` (just the moadim preamble + repo list),
/// so the routine fires on every cron tick and launches an agent with no task —
/// silently burning scheduled runs and the user's agent/API budget (issue #224).
/// Shared by the create and update paths so the REST and MCP surfaces reject it
/// identically, mirroring [`crate::utils::cron::validate_cron`].
pub(super) fn validate_prompt(prompt: &str) -> Result<(), AppError> {
    if prompt.trim().is_empty() {
        return Err(AppError::BadRequest("prompt must not be empty".to_string()));
    }
    Ok(())
}

/// Upper bound on a routine title, in characters, to keep `CLAUDE.md`, crontab
/// comments, iCal `SUMMARY`s, and UI rows from rendering an unbounded string.
pub(super) const MAX_TITLE_LEN: usize = 200;

/// Reject a routine `title` that carries no usable name with `400 Bad Request`.
///
/// `title` is the only required identifying field on a routine, yet it was never
/// content-checked. Two concrete failures follow from a blank or punctuation-only
/// title (issue #226):
///
/// 1. The moadim routine-origin disclosure breaks — `compose_prompt` writes
///    `Routine name: <title>` into the compiled prompt body, so an empty title
///    yields a nameless disclosure the agent cannot satisfy.
/// 2. `slugify` maps any title with no ASCII-alphanumerics (`""`, `"   "`, `"!!!"`)
///    to the constant `"routine"`, so the routine silently takes a slug the user
///    never chose and collides with the next such routine.
///
/// Requiring at least one ASCII-alphanumeric character rejects all three cases at
/// once (it is exactly the condition under which `slugify` falls back). A max
/// length bounds downstream rendering. Shared by the create and update paths so
/// the REST and MCP surfaces reject identically, mirroring [`crate::utils::cron::validate_cron`].
pub(super) fn validate_title(title: &str) -> Result<(), AppError> {
    if !title.chars().any(|ch| ch.is_ascii_alphanumeric()) {
        return Err(AppError::BadRequest(
            "title must contain at least one alphanumeric character".to_string(),
        ));
    }
    if title.trim().chars().count() > MAX_TITLE_LEN {
        return Err(AppError::BadRequest(format!(
            "title must be at most {MAX_TITLE_LEN} characters"
        )));
    }
    Ok(())
}
include!("service_validate_part3.rs");
