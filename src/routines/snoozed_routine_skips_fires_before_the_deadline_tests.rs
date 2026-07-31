
#[test]
fn snoozed_routine_skips_fires_before_the_deadline() {
    // svc_trigger_scheduled refuses to spawn any fire before `snoozed_until`; the feed
    // must not advertise those as real runs either.
    let mut routine = routine_with("r1", "* * * * *", true);
    let now = fixed_now();
    let deadline = now + Duration::minutes(3);
    routine.snoozed_until = Some(u64::try_from(deadline.timestamp()).unwrap());
    // Force the plain-UTC fallback so this test only exercises the snooze-filtering logic, not
    // the TZID-formatting choice (covered separately below).
    let ics = build_ical_with_tz(&[routine], now, 5, None);
    // The first two per-minute fires (00:01, 00:02) fall before the deadline and are
    // dropped; the feed starts at the deadline itself (00:03).
    assert!(ics.contains(&format!(
        "DTSTART:{}\r\n",
        format_utc(deadline.with_timezone(&Utc))
    )));
    let first_dropped = now + Duration::minutes(1);
    assert!(!ics.contains(&format!(
        "DTSTART:{}\r\n",
        format_utc(first_dropped.with_timezone(&Utc))
    )));
    // 5 real VEVENTs (starting at the deadline) plus the truncation marker.
    assert_eq!(count(&ics, "BEGIN:VEVENT"), 6);
}

#[test]
fn skip_runs_drops_the_next_n_fires() {
    // svc_trigger_scheduled decrements skip_runs once per skipped scheduled fire without
    // spawning it; the feed must skip the same leading fires instead of showing them.
    let mut routine = routine_with("r1", "* * * * *", true);
    routine.skip_runs = Some(2);
    let now = fixed_now();
    // Force the plain-UTC fallback so this test only exercises the skip-count logic, not the
    // TZID-formatting choice (covered separately below).
    let ics = build_ical_with_tz(&[routine], now, 3, None);
    let first_kept = now + Duration::minutes(3);
    let first_dropped = now + Duration::minutes(1);
    assert!(ics.contains(&format!(
        "DTSTART:{}\r\n",
        format_utc(first_kept.with_timezone(&Utc))
    )));
    assert!(!ics.contains(&format!(
        "DTSTART:{}\r\n",
        format_utc(first_dropped.with_timezone(&Utc))
    )));
    // 3 real VEVENTs (starting after the 2 skipped fires) plus the truncation marker.
    assert_eq!(count(&ics, "BEGIN:VEVENT"), 4);
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

#[path = "ical_service_tests.rs"]
mod ical_service_tests;

// ── build_ical_with_cap: exact-cap / no-more-fires branch ────────────────────

// ── VTIMEZONE / TZID-qualified DTSTART (issue #387) ──────────────────────────

// `format_utc_offset` tests live in `ical_offset_tests.rs` (split out to keep this file under the
// line cap).

#[allow(
    clippy::missing_docs_in_private_items,
    reason = "split-out module keeps the file under the linecheck limit"
)]
#[path = "text_fields_are_escaped_tests.rs"]
mod text_fields_are_escaped_tests;
