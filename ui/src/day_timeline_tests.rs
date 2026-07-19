//! Host-side unit tests for `fire_times`: the pure cron-to-fire-times logic
//! behind the day timeline, including the midnight-boundary handling and
//! adjacent-day filtering.

use super::*;

fn day() -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 6, 22).expect("valid date")
}

#[test]
fn multiple_fires_within_a_day_are_returned_in_order() {
    let times = fire_times("0 9,12,18 * * *", day());
    assert_eq!(
        times,
        vec![
            NaiveTime::from_hms_opt(9, 0, 0).unwrap(),
            NaiveTime::from_hms_opt(12, 0, 0).unwrap(),
            NaiveTime::from_hms_opt(18, 0, 0).unwrap(),
        ]
    );
}

#[test]
fn midnight_fire_is_attributed_to_the_target_day_not_dropped() {
    // The midnight-minus-one-second seed (see `fire_times`) must still catch
    // an occurrence at exactly 00:00:00 on `day`, not skip past it.
    let times = fire_times("0 0 * * *", day());
    assert_eq!(times, vec![NaiveTime::from_hms_opt(0, 0, 0).unwrap()]);
}

#[test]
fn fires_on_adjacent_days_are_excluded() {
    // Every 12 hours from a fixed anchor lands once just before midnight the
    // previous day and once at noon on `day` — only the noon fire belongs.
    let times = fire_times("0 12 * * *", day());
    assert_eq!(times, vec![NaiveTime::from_hms_opt(12, 0, 0).unwrap()]);
}

#[test]
fn unparseable_schedule_returns_empty() {
    assert!(fire_times("not a cron", day()).is_empty());
    assert!(fire_times("", day()).is_empty());
}

#[test]
fn pathological_schedule_respects_the_max_fires_cap() {
    // Every second, all day, would be 86400 fires without the cap.
    let times = fire_times("* * * * * *", day());
    assert_eq!(times.len(), MAX_FIRES);
}
