
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

#[test]
fn known_host_zone_emits_vtimezone_and_tzid_dtstart() {
    let offset = chrono::FixedOffset::east_opt(3 * 3600).unwrap(); // e.g. Asia/Jerusalem in winter
    let now = fixed_now();
    let ics = build_ical_with_tz(
        &[routine_with("r1", "@daily", true)],
        now,
        MAX_EVENTS_PER_ROUTINE,
        Some(("Asia/Jerusalem".to_string(), offset)),
    );
    assert!(ics.contains("BEGIN:VTIMEZONE\r\n"));
    assert!(ics.contains("TZID:Asia/Jerusalem\r\n"));
    assert!(ics.contains("BEGIN:STANDARD\r\n"));
    assert!(ics.contains("TZOFFSETFROM:+0300\r\n"));
    assert!(ics.contains("TZOFFSETTO:+0300\r\n"));
    assert!(ics.contains("END:STANDARD\r\n"));
    assert!(ics.contains("END:VTIMEZONE\r\n"));
    // Every DTSTART references the VTIMEZONE's TZID with a local (non-`Z`-suffixed) wall-clock
    // time instead of the bare UTC-instant form.
    let events = count(&ics, "BEGIN:VEVENT");
    assert!(events > 0);
    assert_eq!(count(&ics, "DTSTART;TZID=Asia/Jerusalem:"), events);
    // The only bare `DTSTART:` left is the `VTIMEZONE`'s own `STANDARD` sub-component
    // (RFC 5545 requires it there, unrelated to any `VEVENT`'s `DTSTART`).
    assert_eq!(count(&ics, "DTSTART:"), 1);
    // DTSTAMP still stays UTC (RFC 5545 requires it), regardless of the host zone.
    assert!(ics.contains("DTSTAMP:") && ics.contains('Z'));
}

#[test]
fn unresolvable_host_zone_falls_back_to_bare_utc_dtstart() {
    let now = fixed_now();
    let ics = build_ical_with_tz(
        &[routine_with("r1", "@daily", true)],
        now,
        MAX_EVENTS_PER_ROUTINE,
        None,
    );
    assert!(!ics.contains("VTIMEZONE"));
    assert!(!ics.contains("TZID"));
    let events = count(&ics, "BEGIN:VEVENT");
    assert!(events > 0);
    assert_eq!(count(&ics, "DTSTART:"), events);
}

#[test]
fn truncation_marker_also_uses_tzid_dtstart_when_host_zone_known() {
    let offset = chrono::FixedOffset::west_opt(5 * 3600).unwrap(); // e.g. America/New_York in winter
    let now = fixed_now();
    let ics = build_ical_with_tz(
        &[routine_with("r1", "* * * * *", true)],
        now,
        2,
        Some(("America/New_York".to_string(), offset)),
    );
    assert!(ics.contains("-truncated@moadim"));
    // 2 real events + 1 truncation marker, all TZID-qualified.
    assert_eq!(count(&ics, "BEGIN:VEVENT"), 3);
    assert_eq!(count(&ics, "DTSTART;TZID=America/New_York:"), 3);
}
