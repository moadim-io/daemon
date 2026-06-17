#![allow(clippy::missing_docs_in_private_items)]

use super::*;

fn joined(schedule: &str) -> Option<String> {
    cron_to_schtasks(schedule).map(|flags| flags.join(" "))
}

#[test]
fn every_n_minutes() {
    assert_eq!(joined("*/5 * * * *").as_deref(), Some("/SC MINUTE /MO 5"));
    assert_eq!(joined("*/1 * * * *").as_deref(), Some("/SC MINUTE /MO 1"));
}

#[test]
fn step_with_non_wildcard_rest_is_unsupported() {
    // A minute step combined with a constrained hour/day has no schtasks equivalent.
    assert_eq!(joined("*/5 9 * * *"), None);
    // `*/0` is not a valid step.
    assert_eq!(joined("*/0 * * * *"), None);
}

#[test]
fn hourly_at_minute() {
    assert_eq!(
        joined("15 * * * *").as_deref(),
        Some("/SC HOURLY /MO 1 /ST 00:15")
    );
}

#[test]
fn hourly_with_constrained_day_is_unsupported() {
    // minute fixed, hour wildcard, but a day constraint → no clean equivalent.
    assert_eq!(joined("15 * 1 * *"), None);
}

#[test]
fn daily_at_time() {
    assert_eq!(joined("30 9 * * *").as_deref(), Some("/SC DAILY /ST 09:30"));
    assert_eq!(joined("0 0 * * *").as_deref(), Some("/SC DAILY /ST 00:00"));
}

#[test]
fn weekly_weekday_range_and_list() {
    assert_eq!(
        joined("30 9 * * 1-5").as_deref(),
        Some("/SC WEEKLY /D MON,TUE,WED,THU,FRI /ST 09:30")
    );
    assert_eq!(
        joined("0 8 * * 0,6").as_deref(),
        Some("/SC WEEKLY /D SUN,SAT /ST 08:00")
    );
    // Sunday as 7 maps to SUN as well.
    assert_eq!(
        joined("0 8 * * 7").as_deref(),
        Some("/SC WEEKLY /D SUN /ST 08:00")
    );
}

#[test]
fn monthly_day_of_month() {
    assert_eq!(
        joined("0 12 15 * *").as_deref(),
        Some("/SC MONTHLY /D 15 /ST 12:00")
    );
}

#[test]
fn keywords() {
    assert_eq!(joined("@daily").as_deref(), Some("/SC DAILY /ST 00:00"));
    assert_eq!(joined("@midnight").as_deref(), Some("/SC DAILY /ST 00:00"));
    assert_eq!(joined("@reboot").as_deref(), Some("/SC ONSTART"));
    assert_eq!(
        joined("@weekly").as_deref(),
        Some("/SC WEEKLY /D SUN /ST 00:00")
    );
    assert_eq!(
        joined("@monthly").as_deref(),
        Some("/SC MONTHLY /D 1 /ST 00:00")
    );
    assert_eq!(
        joined("@hourly").as_deref(),
        Some("/SC HOURLY /MO 1 /ST 00:00")
    );
    assert_eq!(
        joined("@yearly").as_deref(),
        Some("/SC MONTHLY /M JAN /D 1 /ST 00:00")
    );
    assert_eq!(
        joined("@annually").as_deref(),
        Some("/SC MONTHLY /M JAN /D 1 /ST 00:00")
    );
}

#[test]
fn seven_field_form_reduced() {
    // moadim's 7-field native form (sec min hour dom month dow year) drops sec + year.
    assert_eq!(
        joined("0 30 9 * * * *").as_deref(),
        Some("/SC DAILY /ST 09:30")
    );
}

#[test]
fn out_of_range_fields_are_unsupported() {
    // minute > 59, hour > 23, day-of-month > 31.
    assert_eq!(joined("75 9 * * *"), None);
    assert_eq!(joined("0 25 * * *"), None);
    assert_eq!(joined("0 12 40 * *"), None);
}

#[test]
fn invalid_weekday_fields_are_unsupported() {
    // weekday out of 0..=7.
    assert_eq!(joined("30 9 * * 9"), None);
    // descending range (lo > hi).
    assert_eq!(joined("0 8 * * 5-1"), None);
}

#[test]
fn unsupported_returns_none() {
    // Both DOM and DOW constrained.
    assert_eq!(joined("0 9 1 * 1"), None);
    // Month constraint.
    assert_eq!(joined("0 9 * 6 *"), None);
    // Minute range / list (not a single value or a step).
    assert_eq!(joined("0,30 9 * * *"), None);
    // Wrong field count and garbage.
    assert_eq!(joined("not a schedule"), None);
    assert_eq!(joined("@frequently"), None);
}
