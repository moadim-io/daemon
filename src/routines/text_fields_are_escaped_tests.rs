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
include!("at_cap_with_more_fires_still_in_horizon_adds_truncation_marker_tests.rs");
