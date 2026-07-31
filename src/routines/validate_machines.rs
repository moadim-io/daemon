#![allow(
    clippy::wildcard_imports,
    clippy::too_many_arguments,
    clippy::needless_pass_by_value,
    dead_code,
    reason = "split-out module preserves existing code while keeping files under the linecheck limit"
)]
use super::*;

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
pub(crate) fn validate_machines(machines: &[String]) -> Result<Vec<String>, AppError> {
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
pub(crate) fn normalize_model(model: Option<String>) -> Option<String> {
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
pub(crate) const MAX_GOAL_LINES: usize = 5;

/// Normalize and bound an optional routine `goal`, returning the value to store.
///
/// The goal is a very short statement of *why* a routine exists, rendered into the agent's
/// `prompt.md` as a `## Goal` preamble. It is optional: a `None` or blank (empty/whitespace-only)
/// value clears it (`Ok(None)`). A present goal is trimmed and must span at most
/// [`MAX_GOAL_LINES`] lines, so it stays a glanceable summary rather than a second prompt. Shared
/// by the create and update paths so the REST and MCP surfaces bound it identically.
pub(crate) fn validate_goal(goal: Option<&str>) -> Result<Option<String>, AppError> {
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
