//! Unit tests for the pure schedule math in [`super`]. A fixed `now` keeps every
//! assertion deterministic regardless of the host clock or time zone.

use super::*;
use chrono::{NaiveDate, TimeZone};

/// A fixed reference instant: Sun 2026-06-21 12:00:30 local. Off the minute
/// boundary so `next_fire_after` against a top-of-hour schedule is unambiguous.
fn now() -> DateTime<Local> {
    Local.with_ymd_and_hms(2026, 6, 21, 12, 0, 30).unwrap()
}

#[test]
fn next_fire_after_returns_next_top_of_hour() {
    let then = next_fire_after("0 * * * *", now()).expect("schedule fires");
    let expected = Local.with_ymd_and_hms(2026, 6, 21, 13, 0, 0).unwrap();
    assert_eq!(then, expected);
}

#[test]
fn next_fire_after_rejects_invalid_schedule() {
    assert!(next_fire_after("not a cron", now()).is_none());
    assert!(next_fire_after("", now()).is_none());
}

#[test]
fn fires_within_true_when_next_fire_inside_window() {
    // Next fire is 13:00:00, i.e. 59m30s out — inside a one-hour window.
    assert!(fires_within("0 * * * *", now(), Duration::hours(1)));
}

#[test]
fn fires_within_false_when_next_fire_beyond_window() {
    assert!(!fires_within("0 * * * *", now(), Duration::minutes(30)));
}

#[test]
fn fires_within_false_for_invalid_schedule() {
    assert!(!fires_within("nonsense", now(), Duration::hours(1)));
}

#[test]
fn fmt_until_now_when_due() {
    assert_eq!(fmt_until(now(), now()), "now");
}

#[test]
fn fmt_until_sub_minute() {
    assert_eq!(fmt_until(now(), now() + Duration::seconds(30)), "in <1m");
}

#[test]
fn fmt_until_minutes() {
    assert_eq!(fmt_until(now(), now() + Duration::minutes(5)), "in 5m");
}

#[test]
fn fmt_until_whole_hours() {
    assert_eq!(fmt_until(now(), now() + Duration::hours(2)), "in 2h");
}

#[test]
fn fmt_until_hours_and_minutes() {
    let then = now() + Duration::hours(2) + Duration::minutes(10);
    assert_eq!(fmt_until(now(), then), "in 2h 10m");
}

#[test]
fn fmt_until_days() {
    assert_eq!(fmt_until(now(), now() + Duration::days(3)), "in 3d");
}

#[test]
fn fmt_when_today_is_bare_time() {
    assert_eq!(fmt_when(now(), now() + Duration::hours(1)), "13:00");
}

#[test]
fn fmt_when_tomorrow_is_prefixed() {
    assert_eq!(fmt_when(now(), now() + Duration::days(1)), "tomorrow 12:00");
}

#[test]
fn fmt_when_far_uses_month_and_day() {
    assert_eq!(fmt_when(now(), now() + Duration::days(3)), "Jun 24, 12:00");
}

// ─── Calendar utilities ───────────────────────────────────────────────────────

#[test]
fn month_start_same_month() {
    let today = NaiveDate::from_ymd_opt(2024, 6, 15).unwrap();
    assert_eq!(
        super::month_start(today, 0),
        NaiveDate::from_ymd_opt(2024, 6, 1).unwrap()
    );
}

#[test]
fn month_start_next_month() {
    let today = NaiveDate::from_ymd_opt(2024, 12, 31).unwrap();
    assert_eq!(
        super::month_start(today, 1),
        NaiveDate::from_ymd_opt(2025, 1, 1).unwrap()
    );
}

#[test]
fn month_start_prev_month_year_boundary() {
    let today = NaiveDate::from_ymd_opt(2024, 1, 10).unwrap();
    assert_eq!(
        super::month_start(today, -1),
        NaiveDate::from_ymd_opt(2023, 12, 1).unwrap()
    );
}

#[test]
fn occurrences_per_day_invalid_schedule_returns_none() {
    let today = NaiveDate::from_ymd_opt(2024, 6, 1).unwrap();
    assert!(super::occurrences_per_day("not-a-cron", today).is_none());
}

#[test]
fn occurrences_per_day_daily_fills_one_per_day() {
    let grid_start = NaiveDate::from_ymd_opt(2024, 6, 1).unwrap();
    let counts =
        super::occurrences_per_day("0 12 * * *", grid_start).expect("daily schedule should parse");
    // Every day in the 42-cell grid gets exactly 1 fire
    assert!(counts.iter().all(|&c| c == 1));
}

// ─── Next fires ───────────────────────────────────────────────────────────────

#[test]
fn next_fires_returns_n_sequential_fires() {
    let fires = next_fires("0 * * * *", now(), 3);
    assert_eq!(fires.len(), 3);
    assert_eq!(
        fires[0],
        Local.with_ymd_and_hms(2026, 6, 21, 13, 0, 0).unwrap()
    );
    assert_eq!(
        fires[1],
        Local.with_ymd_and_hms(2026, 6, 21, 14, 0, 0).unwrap()
    );
    assert_eq!(
        fires[2],
        Local.with_ymd_and_hms(2026, 6, 21, 15, 0, 0).unwrap()
    );
}

#[test]
fn next_fires_empty_for_invalid_schedule() {
    assert!(next_fires("not a cron", now(), 5).is_empty());
    assert!(next_fires("", now(), 5).is_empty());
}

#[test]
fn next_fires_zero_n_returns_empty() {
    assert!(next_fires("0 * * * *", now(), 0).is_empty());
}
