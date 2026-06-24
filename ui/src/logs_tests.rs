//! Host-side unit tests for the pure log-viewer helpers in [`super`]:
//! `split_lines` and `line_counts`. No DOM/wasm dependency.

use super::*;

// ─── split_lines ──────────────────────────────────────────────────────────────

#[test]
fn split_lines_empty_text_gives_no_rows() {
    assert!(split_lines("", "").is_empty());
    assert!(split_lines("", "foo").is_empty());
}

#[test]
fn split_lines_blank_query_all_match() {
    let rows = split_lines("alpha\nbeta\ngamma", "");
    assert_eq!(rows.len(), 3);
    assert!(rows.iter().all(|(_, _, m)| *m));
}

#[test]
fn split_lines_whitespace_only_query_all_match() {
    let rows = split_lines("alpha\nbeta", "   ");
    assert_eq!(rows.len(), 2);
    assert!(rows.iter().all(|(_, _, m)| *m));
}

#[test]
fn split_lines_numbers_are_one_based() {
    let rows = split_lines("a\nb\nc", "");
    assert_eq!(rows[0].0, 1);
    assert_eq!(rows[1].0, 2);
    assert_eq!(rows[2].0, 3);
}

#[test]
fn split_lines_content_preserved() {
    let text = "hello world\nfoo bar";
    let rows = split_lines(text, "");
    assert_eq!(rows[0].1, "hello world");
    assert_eq!(rows[1].1, "foo bar");
}

#[test]
fn split_lines_case_insensitive_match() {
    let rows = split_lines("ALPHA\nbeta\nAlPhA", "alpha");
    assert!(rows[0].2, "ALPHA should match 'alpha'");
    assert!(!rows[1].2, "beta should not match 'alpha'");
    assert!(rows[2].2, "AlPhA should match 'alpha'");
}

#[test]
fn split_lines_non_matching_rows_flagged_false() {
    let rows = split_lines("match\nnope\nmatch again", "match");
    assert!(rows[0].2);
    assert!(!rows[1].2);
    assert!(rows[2].2);
}

#[test]
fn split_lines_single_line_match() {
    let rows = split_lines("only line", "only");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].0, 1);
    assert_eq!(rows[0].1, "only line");
    assert!(rows[0].2);
}

#[test]
fn split_lines_single_line_no_match() {
    let rows = split_lines("only line", "other");
    assert_eq!(rows.len(), 1);
    assert!(!rows[0].2);
}

// ─── line_counts ──────────────────────────────────────────────────────────────

#[test]
fn line_counts_empty_text_blank_query() {
    assert_eq!(line_counts("", ""), (0, 0));
}

#[test]
fn line_counts_empty_text_with_query() {
    assert_eq!(line_counts("", "foo"), (0, 0));
}

#[test]
fn line_counts_blank_query_all_match() {
    assert_eq!(line_counts("a\nb\nc", ""), (3, 3));
}

#[test]
fn line_counts_whitespace_query_all_match() {
    assert_eq!(line_counts("a\nb\nc", "  "), (3, 3));
}

#[test]
fn line_counts_partial_match() {
    assert_eq!(line_counts("foo\nbar\nfoo baz", "foo"), (3, 2));
}

#[test]
fn line_counts_no_match() {
    assert_eq!(line_counts("alpha\nbeta", "gamma"), (2, 0));
}

#[test]
fn line_counts_case_insensitive() {
    assert_eq!(line_counts("FOO\nbar\nFoo", "foo"), (3, 2));
}

#[test]
fn line_counts_all_match() {
    assert_eq!(line_counts("foo\nfoo\nfoo", "foo"), (3, 3));
}

#[test]
fn line_counts_single_line_match() {
    assert_eq!(line_counts("hello", "hello"), (1, 1));
}

#[test]
fn line_counts_single_line_no_match() {
    assert_eq!(line_counts("hello", "world"), (1, 0));
}
