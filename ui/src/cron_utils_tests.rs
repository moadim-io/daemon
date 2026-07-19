//! Host-side unit tests for the pure cron helpers in [`super`]: `parse_cron`'s
//! field-count normalization, `describe_cron_live`'s validity/description
//! pairing, and `abstime`'s absolute-timestamp formatting. `reltime` is
//! excluded — it calls `js_sys::Date::now()` and needs a wasm/DOM host, so
//! it isn't host-testable (mirrors the `refresh.rs`/`schedule.rs` split
//! between pure and DOM-touching logic). `abstime` takes its instant as a
//! plain `u64` rather than reading the clock, so it stays host-testable.

use super::*;
use chrono::TimeZone;

#[test]
fn parse_cron_rejects_empty_and_blank() {
    assert!(parse_cron("").is_none());
    assert!(parse_cron("   ").is_none());
}

#[test]
fn parse_cron_accepts_5_field() {
    assert!(parse_cron("0 * * * *").is_some());
}

#[test]
fn parse_cron_accepts_6_field_with_seconds() {
    // 6-field (sec min hour dom month dow) is valid croner syntax as-is, so it
    // parses without normalization, keeping second-level detail in describe().
    let cron = parse_cron("30 0 * * * *").expect("valid 6-field expression");
    assert!(cron.describe().contains("second"));
}

#[test]
fn parse_cron_normalizes_7_field_by_dropping_seconds_and_year() {
    // 7-field (sec min hour dom month dow year) isn't valid croner syntax on
    // its own; parse_cron must strip both the seconds and year fields before
    // handing it to croner, matching the server's normalize_schedule.
    let cron = parse_cron("30 0 12 * * * 2026").expect("normalized to a valid 5-field schedule");
    assert_eq!(cron.describe(), "At 12:00.");
}

#[test]
fn parse_cron_passes_through_at_keywords() {
    assert!(parse_cron("@daily").is_some());
}

#[test]
fn parse_cron_rejects_invalid_expression() {
    assert!(parse_cron("not a cron").is_none());
}

#[test]
fn describe_cron_live_reports_placeholder_for_blank_input() {
    let (valid, description) = describe_cron_live("   ");
    assert!(!valid);
    assert_eq!(description, "— enter a cron expression —");
}

#[test]
fn describe_cron_live_reports_invalid_for_bad_expression() {
    let (valid, description) = describe_cron_live("not a cron");
    assert!(!valid);
    assert_eq!(description, "Invalid cron expression");
}

#[test]
fn describe_cron_live_describes_valid_expression() {
    let (valid, description) = describe_cron_live("0 * * * *");
    assert!(valid);
    assert!(!description.is_empty());
}

#[test]
fn abstime_zero_is_a_placeholder() {
    assert_eq!(abstime(0), "—");
}

#[test]
fn abstime_formats_a_known_instant() {
    // Built in `Local` directly so the expected components match `abstime`'s output regardless
    // of the host's timezone.
    let dt = Local.with_ymd_and_hms(2026, 6, 21, 12, 0, 30).unwrap();
    assert_eq!(abstime(dt.timestamp() as u64), "Jun 21, 2026 12:00");
}

#[test]
fn abstime_falls_back_to_dash_for_an_out_of_range_instant() {
    assert_eq!(abstime(i64::MAX as u64), "—");
}
