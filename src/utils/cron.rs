//! Cron expression normalization and validation, shared by routine scheduling.

use cron_union::{union as union_crons, CronUnion};
use croner::Cron;

use crate::error::AppError;

/// Normalize `expr` to 5-field OS cron format for consistent storage.
///
/// `croner` accepts 5-, 6- (`sec min hour dom month dow`) and 7-field
/// (`sec min hour dom month dow year`) patterns, but the OS crontab only
/// understands 5 fields (`min hour dom month dow`). Both the 6- and 7-field
/// forms carry a leading seconds field, so we strip field 0 (and, for the
/// 7-field form, the trailing year) to land on the 5 middle fields. Without
/// this, a 6-field expression would be written verbatim to the crontab where
/// it is malformed and silently never fires.
///
/// `@keyword` schedules and already-5-field expressions are returned unchanged.
pub(crate) fn normalize_schedule(expr: &str) -> String {
    let trimmed = expr.trim();
    if trimmed.starts_with('@') {
        return trimmed.to_string();
    }
    let fields: Vec<&str> = trimmed.split_ascii_whitespace().collect();
    match fields.len() {
        6 | 7 => fields[1..6].join(" "),
        _ => trimmed.to_string(),
    }
}

/// Parse `expr` as a cron expression, returning `BadRequest` on failure.
///
/// Accepts standard 5-field (`min hour dom month dow`) and `@keyword` formats.
/// 7-field expressions are first normalized to 5-field via [`normalize_schedule`].
pub(crate) fn validate_cron(expr: &str) -> Result<(), AppError> {
    let normalized = normalize_schedule(expr.trim());
    normalized
        .parse::<Cron>()
        .map_err(|err| AppError::BadRequest(format!("invalid cron expression: {err}")))?;
    Ok(())
}

/// Compile `schedule` through `cron-union` when it is a supported cron expression.
///
/// `cron-union` now accepts the same `@keyword` aliases and 7-field schedules the daemon
/// already validates, so this is the fast path for every schedule shape we keep around.
pub(crate) fn compiled_union(schedule: &str) -> Option<CronUnion> {
    let trimmed = schedule.trim();
    if matches!(trimmed, "@reboot" | "@midnight") {
        return None;
    }
    let normalized = normalize_schedule(trimmed);
    union_crons([normalized.as_str()]).ok()
}

#[cfg(test)]
#[path = "cron_tests.rs"]
mod cron_tests;
