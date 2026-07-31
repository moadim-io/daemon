#![allow(
    clippy::wildcard_imports,
    clippy::too_many_arguments,
    clippy::needless_pass_by_value,
    dead_code,
    reason = "split-out module preserves existing code while keeping files under the linecheck limit"
)]
use super::*;

#[test]
fn text_fields_are_escaped() {
    let mut routine = routine_with("r1", "@daily", true);
    routine.title = "a,b;c\\d\ne".to_string();
    let ics = build_ical(&[routine], fixed_now());
    assert!(ics.contains("SUMMARY:a\\,b\\;c\\\\d\\ne\r\n"));
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
