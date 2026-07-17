//! Pure schedule math for the routines "next run" capability: given a cron
//! expression and the current instant, compute the next fire time and format it
//! both as an absolute *when* ("14:30", "tomorrow 09:00", "Jun 24, 09:00") and a
//! relative countdown ("in 5m", "in 2h 10m", "in 3d").
//!
//! Deliberately free of any DOM/wasm dependency so it is unit-testable on the
//! host (see `schedule_tests.rs`). The only inputs are the schedule string and a
//! caller-supplied `now`, keeping every function deterministic.

use chrono::{DateTime, Datelike, Duration, Local, NaiveDate, TimeZone};

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

/// Compute the next `n` fire times for `schedule` strictly after `now`.
/// Returns fewer than `n` items when the schedule has fewer future fires or is
/// empty/invalid.
pub(crate) fn next_fires(schedule: &str, now: DateTime<Local>, n: usize) -> Vec<DateTime<Local>> {
    let Some(cron) = parse_cron(schedule) else {
        return vec![];
    };
    cron.iter_after(now).take(n).collect()
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

// ─── Calendar grid utilities ──────────────────────────────────────────────────
//
// Used by the routines calendar view.

/// Day-of-week headers for the calendar grid.
pub(crate) const WEEKDAYS: [&str; 7] = ["SUN", "MON", "TUE", "WED", "THU", "FRI", "SAT"];

/// Full month names for the calendar navigation header.
pub(crate) const CAL_MONTHS: [&str; 12] = [
    "JANUARY",
    "FEBRUARY",
    "MARCH",
    "APRIL",
    "MAY",
    "JUNE",
    "JULY",
    "AUGUST",
    "SEPTEMBER",
    "OCTOBER",
    "NOVEMBER",
    "DECEMBER",
];

/// Cells in the month grid: 6 weeks × 7 days, always, so the layout never reflows.
pub(crate) const GRID_CELLS: usize = 42;

/// Upper bound on fire-time iterations per schedule across the visible grid.
pub(crate) const MAX_OCCURRENCES: usize = 4000;

/// First day of the month `offset` months away from the month containing `today`.
pub(crate) fn month_start(today: NaiveDate, offset: i32) -> NaiveDate {
    let total = today.year() * 12 + today.month0() as i32 + offset;
    let year = total.div_euclid(12);
    let month0 = total.rem_euclid(12) as u32;
    NaiveDate::from_ymd_opt(year, month0 + 1, 1).unwrap_or(today)
}

/// Fire times of `schedule` that land on `date` (the caller's local calendar
/// day), formatted `"HH:MM"` in ascending order. Empty for an empty/invalid
/// schedule or a day with no fires.
pub(crate) fn fires_on_day(schedule: &str, date: NaiveDate) -> Vec<String> {
    let Some(cron) = parse_cron(schedule) else {
        return vec![];
    };
    let Some(start_naive) = date.and_hms_opt(0, 0, 0) else {
        return vec![];
    };
    let Some(start) = Local.from_local_datetime(&start_naive).earliest() else {
        return vec![];
    };
    let start = start - Duration::seconds(1);
    cron.iter_after(start)
        .take_while(|dt| dt.date_naive() == date)
        .map(|dt| dt.format("%H:%M").to_string())
        .collect()
}

/// Fire counts per grid cell for `schedule` over `[grid_start, grid_start + 42 days)`.
pub(crate) fn occurrences_per_day(
    schedule: &str,
    grid_start: NaiveDate,
) -> Option<[u32; GRID_CELLS]> {
    let cron = parse_cron(schedule)?;
    let start_naive = grid_start.and_hms_opt(0, 0, 0)?;
    let start = Local
        .from_local_datetime(&start_naive)
        .earliest()?
        .checked_sub_signed(Duration::seconds(1))?;
    let mut counts = [0u32; GRID_CELLS];
    for dt in cron.iter_after(start).take(MAX_OCCURRENCES) {
        let day = (dt.date_naive() - grid_start).num_days();
        if day < 0 {
            continue;
        }
        if day as usize >= GRID_CELLS {
            break;
        }
        counts[day as usize] += 1;
    }
    Some(counts)
}

#[cfg(test)]
#[path = "schedule_tests.rs"]
mod schedule_tests;
