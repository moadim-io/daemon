//! Pure schedule math for the cron-jobs "next run" capability: given a cron
//! expression and the current instant, compute the next fire time and format it
//! both as an absolute *when* ("14:30", "tomorrow 09:00", "Jun 24, 09:00") and a
//! relative countdown ("in 5m", "in 2h 10m", "in 3d").
//!
//! Deliberately free of any DOM/wasm dependency so it is unit-testable on the
//! host (see `schedule_tests.rs`). The only inputs are the schedule string and a
//! caller-supplied `now`, keeping every function deterministic.

use chrono::{DateTime, Datelike, Duration, Local};

use crate::parse_cron;

/// Month abbreviations for absolute fire times more than a day out.
const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

/// The next fire time strictly after `now` for `schedule`, or `None` when the
/// expression is empty/invalid or never fires again.
pub(crate) fn next_fire_after(schedule: &str, now: DateTime<Local>) -> Option<DateTime<Local>> {
    parse_cron(schedule)?.iter_after(now).next()
}

/// `true` when `schedule`'s next fire lands within `window` of `now`.
pub(crate) fn fires_within(schedule: &str, now: DateTime<Local>, window: Duration) -> bool {
    match next_fire_after(schedule, now) {
        Some(then) => then - now <= window,
        None => false,
    }
}

/// Format `then` as a relative countdown from `now`: "in 5m", "in 2h 10m",
/// "in 3d", "in <1m", or "now" when it is due this instant.
pub(crate) fn fmt_until(now: DateTime<Local>, then: DateTime<Local>) -> String {
    let secs = (then - now).num_seconds();
    if secs <= 0 {
        return "now".into();
    }
    let mins = secs / 60;
    if mins < 1 {
        return "in <1m".into();
    }
    if mins < 60 {
        return format!("in {mins}m");
    }
    let hours = mins / 60;
    if hours < 24 {
        let rem = mins % 60;
        return if rem == 0 {
            format!("in {hours}h")
        } else {
            format!("in {hours}h {rem}m")
        };
    }
    let days = hours / 24;
    format!("in {days}d")
}

/// Format `then` as an absolute fire time relative to `now`'s calendar day:
/// "14:30" when today, "tomorrow 09:00" the next day, else "Jun 24, 09:00".
pub(crate) fn fmt_when(now: DateTime<Local>, then: DateTime<Local>) -> String {
    let hm = then.format("%H:%M").to_string();
    let days = (then.date_naive() - now.date_naive()).num_days();
    if days == 0 {
        hm
    } else if days == 1 {
        format!("tomorrow {hm}")
    } else {
        let month = MONTHS[then.month0() as usize];
        let day = then.day();
        format!("{month} {day}, {hm}")
    }
}

#[cfg(test)]
#[path = "schedule_tests.rs"]
mod schedule_tests;
