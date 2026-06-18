#![allow(clippy::missing_docs_in_private_items)]

use super::*;
use crate::routines::model::{new_store, Routine};
use chrono::{Local, TimeZone};

fn routine_with(id: &str, schedule: &str, enabled: bool) -> Routine {
    Routine {
        id: id.to_string(),
        schedule: schedule.to_string(),
        title: "My Routine".to_string(),
        agent: "claude".to_string(),
        prompt: "do the thing".to_string(),
        repositories: vec![],
        enabled,
        source: "managed".to_string(),
        created_at: 0,
        updated_at: 0,
        last_triggered_at: None,
        ttl_secs: None,
    }
}

fn fixed_now() -> chrono::DateTime<Local> {
    Local.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap()
}

fn count(haystack: &str, needle: &str) -> usize {
    haystack.matches(needle).count()
}

#[test]
fn empty_feed_has_only_calendar_wrapper() {
    let ics = build_ical(&[], fixed_now());
    assert!(ics.starts_with("BEGIN:VCALENDAR\r\n"));
    assert!(ics.contains("VERSION:2.0\r\n"));
    assert!(ics.contains("PRODID:-//moadim//routines//EN\r\n"));
    assert!(ics.contains("X-WR-CALNAME:Moadim Routines\r\n"));
    assert!(ics.ends_with("END:VCALENDAR\r\n"));
    assert_eq!(count(&ics, "BEGIN:VEVENT"), 0);
}

#[test]
fn enabled_daily_routine_yields_events_within_horizon() {
    let ics = build_ical(&[routine_with("r1", "@daily", true)], fixed_now());
    let events = count(&ics, "BEGIN:VEVENT");
    // ~30 daily fires fall inside the 30-day horizon; allow slack for DST edges.
    assert!(events >= 28, "expected ~30 daily events, got {events}");
    assert!(ics.contains("SUMMARY:My Routine\r\n"));
    assert!(ics.contains("DESCRIPTION:do the thing (agent: claude)\r\n"));
    assert!(ics.contains("UID:r1-"));
    assert!(ics.contains("@moadim\r\n"));
    assert!(ics.contains("DTSTART:"));
    assert!(ics.contains("DTSTAMP:"));
}

#[test]
fn disabled_routine_contributes_nothing() {
    let ics = build_ical(&[routine_with("r1", "@daily", false)], fixed_now());
    assert_eq!(count(&ics, "BEGIN:VEVENT"), 0);
}

#[test]
fn unparseable_schedule_is_skipped() {
    let ics = build_ical(&[routine_with("r1", "@reboot", true)], fixed_now());
    assert_eq!(count(&ics, "BEGIN:VEVENT"), 0);
}

#[test]
fn high_frequency_schedule_is_capped() {
    let ics = build_ical(&[routine_with("r1", "* * * * *", true)], fixed_now());
    assert_eq!(count(&ics, "BEGIN:VEVENT"), 100);
}

#[test]
fn text_fields_are_escaped() {
    let mut routine = routine_with("r1", "@daily", true);
    routine.title = "a,b;c\\d\ne".to_string();
    let ics = build_ical(&[routine], fixed_now());
    assert!(ics.contains("SUMMARY:a\\,b\\;c\\\\d\\ne\r\n"));
}

#[test]
fn carriage_returns_are_normalized() {
    let mut routine = routine_with("r1", "@daily", true);
    // A lone CR and a CRLF, as pasted Windows / multi-line text produces.
    routine.title = "a\rb\r\nc".to_string();
    routine.prompt = "x\r\ny".to_string();
    let ics = build_ical(&[routine], fixed_now());

    // Both the lone CR and the CRLF collapse to the same escaped newline as a bare LF.
    assert!(ics.contains("SUMMARY:a\\nb\\nc\r\n"));
    assert!(ics.contains("DESCRIPTION:x\\ny (agent: claude)\r\n"));

    // No raw CR survives except as part of a structural CRLF line terminator.
    assert!(
        !ics.replace("\r\n", "").contains('\r'),
        "feed contains a stray carriage return"
    );
}

#[test]
fn svc_ical_reads_store() {
    let store = new_store();
    store
        .lock()
        .unwrap()
        .insert("r1".to_string(), routine_with("r1", "@daily", true));
    let ics = svc_ical(&store);
    assert!(ics.starts_with("BEGIN:VCALENDAR"));
    assert!(ics.contains("BEGIN:VEVENT"));
}
