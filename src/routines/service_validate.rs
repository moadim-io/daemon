//! Field-validation helpers shared by [`super::svc_create`] and [`super::svc_update`].

use crate::error::AppError;
use crate::routines::agents::{available_agents, load_agent_command, AgentLoadError};
use crate::routines::command::{is_valid_env_key, validate_placeholders};
use crate::routines::model::Repository;

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

/// Reject `repositories` entries whose URL (or set branch) is empty/whitespace-only, and return a
/// normalized copy with surrounding whitespace trimmed.
///
/// `repository` is a free-form string rendered verbatim into the agent's `prompt.compiled.local.md` preamble by
/// `compose_prompt` (see #241), so a blank or padded entry yields a broken `- ` clone bullet. An
/// empty list is valid — this only guards the contents of non-empty entries. Mirrors the
/// `validate_cron` / `validate_agent` boundary checks for the other routine fields (#224/#226).
pub(super) fn validate_repositories(repos: &[Repository]) -> Result<Vec<Repository>, AppError> {
    let mut normalized = Vec::with_capacity(repos.len());
    for (index, repo) in repos.iter().enumerate() {
        let repository = repo.repository.trim();
        if repository.is_empty() {
            return Err(AppError::BadRequest(format!(
                "repositories[{index}].repository must not be empty or whitespace-only"
            )));
        }
        let branch = match &repo.branch {
            Some(branch) => {
                let trimmed = branch.trim();
                if trimmed.is_empty() {
                    return Err(AppError::BadRequest(format!(
                        "repositories[{index}].branch must not be empty or whitespace-only when set"
                    )));
                }
                Some(trimmed.to_string())
            }
            None => None,
        };
        normalized.push(Repository {
            repository: repository.to_string(),
            branch,
        });
    }
    Ok(normalized)
}

/// Reject blank (empty/whitespace-only) `tags` entries and return a normalized copy with each tag
/// trimmed and duplicates collapsed (first occurrence kept).
///
/// Tags are free-form labels for grouping routines; an empty list is valid. This only guards the
/// contents of non-empty entries, mirroring [`validate_repositories`]: a blank label carries no
/// meaning and would render as an empty chip, so it is refused at edit time rather than stored.
/// The dedup step mirrors [`validate_machines`]: left unchecked, `["nightly", "nightly"]` (or a
/// padded repeat like `" nightly "`) persists and renders as a doubled chip in the routine row and
/// an inflated (if harmless) entry in the tag facet's per-tag matching, for a label that names one
/// concept once.
pub(super) fn validate_tags(tags: &[String]) -> Result<Vec<String>, AppError> {
    let mut normalized: Vec<String> = Vec::with_capacity(tags.len());
    for (index, tag) in tags.iter().enumerate() {
        let trimmed = tag.trim();
        if trimmed.is_empty() {
            return Err(AppError::BadRequest(format!(
                "tags[{index}] must not be empty or whitespace-only"
            )));
        }
        if !normalized.iter().any(|existing| existing == trimmed) {
            normalized.push(trimmed.to_string());
        }
    }
    Ok(normalized)
}

/// Reject an `env` map with an invalid key or a value that could inject an extra shell statement.
///
/// Keys must be POSIX-portable shell identifiers ([`is_valid_env_key`]) —
/// [`crate::routines::command::build_routine_command`]
/// emits each entry as a literal `export KEY=<shell-quoted value>` statement, so a key outside that
/// shape (e.g. containing `=`, whitespace, or `;`) would either fail to export or, unquoted as it
/// must be for `export NAME=...` syntax to work, let a crafted key break out of the statement.
/// Values are shell-quoted ([`crate::routines::command::shell_quote`]) so most characters are safe,
/// but a value containing a newline still splits the single-line, `;`-joined launch command into
/// two shell statements — an injection distinct from anything quoting can neutralize — so newlines
/// are rejected outright (#408).
pub(super) fn validate_env(
    env: &std::collections::HashMap<String, String>,
) -> Result<(), AppError> {
    for (key, value) in env {
        if !is_valid_env_key(key) {
            return Err(AppError::BadRequest(format!(
                "env key {key:?} is invalid; keys must match [A-Za-z_][A-Za-z0-9_]*"
            )));
        }
        if value.contains('\n') || value.contains('\r') {
            return Err(AppError::BadRequest(format!(
                "env value for key {key:?} must not contain newline characters"
            )));
        }
    }
    Ok(())
}

/// Reject blank (empty/whitespace-only) `machines` entries and return a normalized copy with each
/// entry trimmed and duplicates collapsed (first occurrence kept).
///
/// `machine::targets` matches this list against the resolved machine name, either by exact string
/// equality or, for an entry containing `*`, as a glob (see #600, #1393). Left unvalidated, a
/// whitespace-padded or typo'd entry can never match anything, and a
/// non-empty list of *only* empty-string entries slips past the dormant-routine warning — which fires
/// solely on `machines.is_empty()` — leaving a routine that runs nowhere with no warning at all.
/// Trimming and rejecting blanks mirrors `validate_repositories`/`validate_tags`; the extra dedup
/// step additionally stops `"host"` and `" host "` from persisting as if they targeted two machines.
pub(super) fn validate_machines(machines: &[String]) -> Result<Vec<String>, AppError> {
    let mut normalized: Vec<String> = Vec::with_capacity(machines.len());
    for (index, machine) in machines.iter().enumerate() {
        let trimmed = machine.trim();
        if trimmed.is_empty() {
            return Err(AppError::BadRequest(format!(
                "machines[{index}] must not be empty or whitespace-only"
            )));
        }
        if !normalized.iter().any(|existing| existing == trimmed) {
            normalized.push(trimmed.to_string());
        }
    }
    Ok(normalized)
}

/// Normalize an optional model ID: trims it and collapses blank/whitespace-only input to `None`, so
/// a cleared text field on the create/edit form is stored as "no override" rather than an empty
/// string.
pub(super) fn normalize_model(model: Option<String>) -> Option<String> {
    model.and_then(|model| {
        let trimmed = model.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

/// Maximum number of lines a routine `goal` may span. The goal is meant to be a glanceable "why"
/// rendered as a `## Goal` preamble in `prompt.md`, not a second prompt, so it is capped short.
const MAX_GOAL_LINES: usize = 5;

/// Normalize and bound an optional routine `goal`, returning the value to store.
///
/// The goal is a very short statement of *why* a routine exists, rendered into the agent's
/// `prompt.md` as a `## Goal` preamble. It is optional: a `None` or blank (empty/whitespace-only)
/// value clears it (`Ok(None)`). A present goal is trimmed and must span at most
/// [`MAX_GOAL_LINES`] lines, so it stays a glanceable summary rather than a second prompt. Shared
/// by the create and update paths so the REST and MCP surfaces bound it identically.
pub(super) fn validate_goal(goal: Option<&str>) -> Result<Option<String>, AppError> {
    let Some(trimmed) = goal.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    if trimmed.lines().count() > MAX_GOAL_LINES {
        return Err(AppError::BadRequest(format!(
            "goal must be at most {MAX_GOAL_LINES} lines"
        )));
    }
    Ok(Some(trimmed.to_string()))
}
