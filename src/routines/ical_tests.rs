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
        machines: vec![],
        enabled,
        source: "managed".to_string(),
        created_at: 0,
        updated_at: 0,
        last_manual_trigger_at: None,
        last_scheduled_trigger_at: None,
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
    // Fire times are momentary triggers, not busy blocks: every event is
    // TRANSPARENT so subscribers aren't marked BUSY (one per VEVENT).
    assert!(ics.contains("TRANSP:TRANSPARENT\r\n"));
    assert_eq!(count(&ics, "TRANSP:TRANSPARENT"), events);
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
    // 100 real events plus one trailing truncation-marker VEVENT (see below).
    assert_eq!(count(&ics, "BEGIN:VEVENT"), 101);
}

#[test]
fn truncated_schedule_emits_marker_event() {
    let ics = build_ical(&[routine_with("r1", "* * * * *", true)], fixed_now());
    // The cap is surfaced, not silent: a distinctly-UID'd marker VEVENT is appended.
    assert!(ics.contains("UID:r1-truncated@moadim\r\n"));
    assert!(ics.contains("SUMMARY:⚠ My Routine (schedule truncated)\r\n"));
    // The DESCRIPTION is long enough to be line-folded; unfold before matching its prose.
    let unfolded = ics.replace("\r\n ", "");
    assert!(unfolded.contains("only the first 100 of more upcoming runs"));
    // Exactly one marker, regardless of how far over the cap the routine fires.
    assert_eq!(count(&ics, "-truncated@moadim"), 1);
}

#[test]
fn untruncated_schedule_has_no_marker() {
    // A daily routine stays well under the cap, so no truncation marker is emitted.
    let ics = build_ical(&[routine_with("r1", "@daily", true)], fixed_now());
    assert!(!ics.contains("-truncated@moadim"));
    assert!(!ics.contains("schedule truncated"));
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
fn carriage_returns_crlf_and_lone_cr_normalized() {
    let mut routine = routine_with("r1", "@daily", true);
    // A pasted CRLF plus a lone CR — neither may leak a raw `\r` into the feed.
    routine.title = "a\r\nb\rc".to_string();
    let ics = build_ical(&[routine], fixed_now());
    assert!(ics.contains("SUMMARY:a\\nb\\nc\r\n"));
    // The only raw CRs left are the structural CRLF line terminators.
    assert!(!ics.replace("\r\n", "").contains('\r'));
}

#[test]
fn description_summarizes_long_multiline_prompt() {
    let mut routine = routine_with("r1", "* * * * *", true);
    routine.prompt = format!("First line of the plan\n{}", "x".repeat(5000));
    let ics = build_ical(&[routine], fixed_now());
    // Only the first line is shown, with an ellipsis marking the elided remainder.
    assert!(ics.contains("DESCRIPTION:First line of the plan… (agent: claude)\r\n"));
    // The multi-KB tail never reaches the feed, even once.
    assert!(!ics.contains("xxxxxxxxxx"));
}

#[test]
fn description_truncates_overlong_single_line() {
    let mut routine = routine_with("r1", "@daily", true);
    routine.prompt = "a".repeat(500);
    let ics = build_ical(&[routine], fixed_now());
    // Unfold continuation lines (strip CRLF + leading space) before inspecting the
    // logical content; the long prompt summary causes the DESCRIPTION to be folded
    // across multiple physical lines per RFC 5545 §3.1.
    let unfolded = ics.replace("\r\n ", "");
    let mut saw_description = false;
    for line in unfolded
        .split("\r\n")
        .filter(|entry| entry.starts_with("DESCRIPTION:"))
    {
        saw_description = true;
        assert!(
            line.chars().count() < 200,
            "DESCRIPTION not bounded: {line}"
        );
        assert!(line.ends_with("… (agent: claude)"));
    }
    assert!(saw_description);
}

#[test]
fn description_handles_blank_prompt() {
    let mut routine = routine_with("r1", "@daily", true);
    routine.prompt = "   \n  ".to_string();
    let ics = build_ical(&[routine], fixed_now());
    assert!(ics.contains("DESCRIPTION: (agent: claude)\r\n"));
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
    // Prompt "x\r\ny" is multi-line; prompt_summary takes the first non-empty line ("x")
    // and appends "…" because further lines exist. The CR/CRLF never reach the feed.
    assert!(ics.contains("DESCRIPTION:x… (agent: claude)\r\n"));

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

#[test]
fn svc_ical_routine_filters_to_one_routine() {
    // Two enabled routines in the store; the filtered feed contains only the requested
    // one's events, and the calendar is named after that routine (issue #263).
    let store = new_store();
    {
        let mut lock = store.lock().unwrap();
        let mut keep = routine_with("keep", "@daily", true);
        keep.title = "Keep Me".to_string();
        lock.insert("keep".to_string(), keep);
        let mut other = routine_with("other", "@daily", true);
        other.title = "Other".to_string();
        lock.insert("other".to_string(), other);
    }
    let ics = svc_ical_routine(&store, "keep");
    assert!(ics.contains("UID:keep-"));
    assert!(!ics.contains("UID:other-"));
    assert!(ics.contains("SUMMARY:Keep Me\r\n"));
    // Calendar is named after the routine, not the generic all-routines name.
    assert!(ics.contains("X-WR-CALNAME:Keep Me\r\n"));
    assert!(!ics.contains("X-WR-CALNAME:Moadim Routines\r\n"));
}

#[test]
fn svc_ical_routine_unknown_id_is_well_formed_empty_calendar() {
    // An unknown id is not an error: a valid, empty VCALENDAR with the default name.
    let store = new_store();
    store
        .lock()
        .unwrap()
        .insert("r1".to_string(), routine_with("r1", "@daily", true));
    let ics = svc_ical_routine(&store, "does-not-exist");
    assert!(ics.starts_with("BEGIN:VCALENDAR\r\n"));
    assert!(ics.contains("X-WR-CALNAME:Moadim Routines\r\n"));
    assert!(ics.ends_with("END:VCALENDAR\r\n"));
    assert_eq!(count(&ics, "BEGIN:VEVENT"), 0);
}

#[test]
fn build_ical_skips_all_routines_when_globally_locked() {
    let dir = std::env::temp_dir().join(format!("moadim-icallock-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).expect("create temp home");
    // SAFETY: single-threaded test execution (RUST_TEST_THREADS=1).
    unsafe {
        std::env::set_var("MOADIM_HOME_OVERRIDE", &dir);
    }
    let lock_path = crate::paths::global_lock_path();
    if let Some(parent) = lock_path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(&lock_path, b"").unwrap();

    let routine = routine_with("rl", "@daily", true);
    let ics = build_ical(&[routine], fixed_now());
    assert!(
        !ics.contains("BEGIN:VEVENT"),
        "globally locked feed must have no events"
    );

    // SAFETY: cleanup.
    unsafe {
        std::env::remove_var("MOADIM_HOME_OVERRIDE");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

// ── build_ical_with_cap: exact-cap / no-more-fires branch ────────────────────

#[test]
fn at_cap_with_no_further_fires_in_horizon_adds_no_truncation_marker() {
    // Use cap=1 with a once-per-year schedule so the iterator is exhausted after emitting
    // exactly 1 event: emitted == max_events, but fires.next() returns None because the
    // next occurrence is a full year later (well beyond the 30-day horizon).
    // This exercises the `if emitted == max_events { if let Some(next) = fires.next() { … } }`
    // path where the inner if-let arm is NOT taken — the closing `}` of the outer if is reached
    // without ever appending the truncation-marker VEVENT.
    let routine = routine_with("r1", "0 0 2 1 *", true); // fires on 2 January at midnight
    let now = fixed_now(); // 2026-01-01 00:00:00 local
                           // Only 2026-01-02 00:00:00 falls within the 30-day horizon; the next fire is 2027-01-02.
    let ics = build_ical_with_cap(&[routine], now, 1);
    // Exactly one real VEVENT; no truncation-marker VEVENT.
    assert_eq!(count(&ics, "BEGIN:VEVENT"), 1);
    assert!(
        !ics.contains("-truncated@moadim"),
        "no truncation marker expected"
    );
}

#[test]
fn at_cap_with_more_fires_still_in_horizon_adds_truncation_marker() {
    // Counterpart: a daily schedule gives ~30 fires; with cap=2 the third fire is still inside
    // the horizon so fires.next() returns Some and the truncation marker IS appended.
    let routine = routine_with("r1", "0 0 * * *", true); // fires daily at midnight
    let now = fixed_now();
    let ics = build_ical_with_cap(&[routine], now, 2);
    // 2 real VEVENTs + 1 truncation-marker VEVENT.
    assert_eq!(count(&ics, "BEGIN:VEVENT"), 3);
    assert!(
        ics.contains("-truncated@moadim"),
        "truncation marker expected"
    );
}
