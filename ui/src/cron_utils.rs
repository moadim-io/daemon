//! Cron-expression parsing and formatting helpers shared across the routines,
//! schedule, and heatmap views. Split out of `main.rs` to keep that file under
//! the line-count gate; re-exported from the crate root so existing
//! `crate::parse_cron`-style paths keep working unchanged.

use croner::Cron;

/// Parse a cron expression into a `Cron`, normalizing the 7-field
/// (sec min hour dom month dow year) form to 5-field to match server behaviour.
/// Returns `None` for empty or invalid expressions.
pub(crate) fn parse_cron(expr: &str) -> Option<Cron> {
    let s = expr.trim();
    if s.is_empty() {
        return None;
    }
    let normalized = if s.starts_with('@') {
        s.to_string()
    } else {
        let parts: Vec<&str> = s.split_whitespace().collect();
        if parts.len() == 7 {
            parts[1..6].join(" ")
        } else {
            s.to_string()
        }
    };
    normalized.parse::<Cron>().ok()
}

/// Returns (is_valid, human description) for a cron expression.
pub(crate) fn describe_cron_live(expr: &str) -> (bool, String) {
    if expr.trim().is_empty() {
        return (false, "— enter a cron expression —".into());
    }
    match parse_cron(expr) {
        Some(cron) => (true, cron.describe()),
        None => (false, "Invalid cron expression".into()),
    }
}

pub(crate) fn reltime(ts: u64) -> String {
    if ts == 0 {
        return "—".into();
    }
    let now = (js_sys::Date::now() / 1000.0) as u64;
    let diff = now.saturating_sub(ts);
    if diff < 60 {
        "just now".into()
    } else if diff < 3_600 {
        format!("{}m ago", diff / 60)
    } else if diff < 86_400 {
        format!("{}h ago", diff / 3_600)
    } else {
        format!("{}d ago", diff / 86_400)
    }
}
