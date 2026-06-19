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
        last_manual_trigger_at: None,
        ttl_secs: None,
        max_runtime_secs: None,
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
    assert!(ics.contains("REFRESH-INTERVAL;VALUE=DURATION:PT1H\r\n"));
    assert!(ics.contains("X-PUBLISHED-TTL:PT1H\r\n"));
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

/// Assert no physical line in `ics` exceeds 75 octets (excluding the CRLF).
fn assert_all_lines_within_75_octets(ics: &str) {
    for line in ics.split("\r\n") {
        assert!(
            line.len() <= 75,
            "line exceeds 75 octets ({}): {line:?}",
            line.len()
        );
    }
}

#[test]
fn short_value_is_left_unfolded() {
    assert_eq!(fold_line("SUMMARY:hello"), "SUMMARY:hello");
    // exactly 75 octets stays on one line
    let exact = "A".repeat(75);
    assert_eq!(fold_line(&exact), exact);
}

#[test]
fn long_line_is_folded_with_leading_space() {
    let line = format!("DESCRIPTION:{}", "x".repeat(200));
    let folded = fold_line(&line);
    let physical: Vec<&str> = folded.split("\r\n").collect();
    assert!(physical.len() > 1, "expected multiple folded lines");
    assert!(physical[0].len() <= 75);
    for cont in &physical[1..] {
        assert!(
            cont.starts_with(' '),
            "continuation must start with a space"
        );
        assert!(cont.len() <= 75, "continuation exceeds 75 octets");
    }
    // unfolding (strip CRLF + single leading space) restores the original
    let unfolded = folded.replace("\r\n ", "");
    assert_eq!(unfolded, line);
}

#[test]
fn fold_never_splits_multibyte_character() {
    // 'é' is 2 octets in UTF-8; place a run straddling the 75-octet boundary.
    let line = format!("SUMMARY:{}", "é".repeat(80));
    let folded = fold_line(&line);
    for cont in folded.split("\r\n") {
        assert!(cont.len() <= 75);
    }
    // Every physical line must be valid UTF-8 with no replacement chars,
    // i.e. no character was split mid-codepoint.
    let unfolded = folded.replace("\r\n ", "");
    assert_eq!(unfolded, line);
    assert!(!folded.contains('\u{FFFD}'));
}

#[test]
fn feed_with_long_prompt_is_fully_folded() {
    let mut routine = routine_with("r1", "@daily", true);
    routine.prompt = "lorem ipsum dolor sit amet ".repeat(20);
    routine.title = "A very long routine title ".repeat(5);
    let ics = build_ical(&[routine], fixed_now());
    assert_all_lines_within_75_octets(&ics);
    // DESCRIPTION was long enough to require at least one continuation line.
    assert!(ics.contains("\r\n "), "expected folded continuation lines");
}

#[test]
fn carriage_returns_are_normalized() {
    let mut routine = routine_with("r1", "@daily", true);
    // A pasted CRLF plus a lone CR — neither may leak a raw `\r` into the feed.
    routine.title = "a\r\nb\rc".to_string();
    let ics = build_ical(&[routine], fixed_now());
    assert!(ics.contains("SUMMARY:a\\nb\\nc\r\n"));
    // The only raw CRs left are the structural CRLF line terminators.
    assert!(!ics.replace("\r\n", "").contains('\r'));
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
