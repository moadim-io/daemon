//! Host-side unit tests for the log viewer's pure helpers.
//! No DOM / wasm dependency — mirrors the schedule/overview/cron-jobs test conventions.

use super::*;

// ── filter_lines ─────────────────────────────────────────────────────────────

#[test]
fn empty_query_returns_all_lines_numbered_from_one() {
    let out = filter_lines("alpha\nbeta\ngamma", "");
    assert_eq!(out, vec![(1, "alpha"), (2, "beta"), (3, "gamma")]);
}

#[test]
fn blank_query_whitespace_returns_all() {
    assert_eq!(filter_lines("x\ny", "   ").len(), 2);
}

#[test]
fn query_filters_case_insensitively() {
    let content = "INFO: server started\nERROR: connection refused\nWARN: retrying";
    let out = filter_lines(content, "ERROR");
    assert_eq!(out.len(), 1);
    assert_eq!(out[0], (2, "ERROR: connection refused"));
}

#[test]
fn query_preserves_original_line_numbers() {
    let out = filter_lines("line 1\nline 2\nline 3\nline 4", "line 3");
    assert_eq!(out, vec![(3, "line 3")]);
}

#[test]
fn no_match_returns_empty_vec() {
    assert!(filter_lines("hello world", "zzz").is_empty());
}

#[test]
fn single_line_exact_match() {
    let out = filter_lines("hello", "hello");
    assert_eq!(out, vec![(1, "hello")]);
}

#[test]
fn single_line_no_match() {
    assert!(filter_lines("hello", "world").is_empty());
}

#[test]
fn trailing_newline_does_not_produce_extra_numbered_line() {
    // "alpha\n" has two splits in .lines() only if there are two logical lines;
    // std::str::Lines skips a trailing empty segment, so count stays at 1.
    let out = filter_lines("alpha\n", "");
    assert_eq!(out.len(), 1);
}

// ── match_count ──────────────────────────────────────────────────────────────

#[test]
fn blank_query_count_equals_total() {
    let (hits, total) = match_count("a\nb\nc", "");
    assert_eq!(hits, 3);
    assert_eq!(total, 3);
}

#[test]
fn whitespace_query_treated_as_blank() {
    let (hits, total) = match_count("a\nb", "   ");
    assert_eq!(hits, total);
}

#[test]
fn partial_match_count() {
    let (hits, total) = match_count("foo\nbar\nfoo bar", "foo");
    assert_eq!(hits, 2);
    assert_eq!(total, 3);
}

#[test]
fn no_match_count_is_zero() {
    let (hits, total) = match_count("a\nb", "z");
    assert_eq!(hits, 0);
    assert_eq!(total, 2);
}

#[test]
fn match_count_is_case_insensitive() {
    let (hits, _) = match_count("ERROR\nerror\nOk", "error");
    assert_eq!(hits, 2);
}

#[test]
fn empty_content_gives_zero_total() {
    let (hits, total) = match_count("", "anything");
    assert_eq!(hits, 0);
    assert_eq!(total, 0);
}

#[test]
fn empty_content_blank_query() {
    let (hits, total) = match_count("", "");
    assert_eq!(hits, 0);
    assert_eq!(total, 0);
}
