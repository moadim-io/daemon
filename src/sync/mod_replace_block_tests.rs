#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use super::*;

// ─── replace_block_with ──────────────────────────────────────────────────────

const TEST_BEGIN: &str = "# BEGIN TEST";
const TEST_END: &str = "# END TEST";

#[test]
fn replace_block_with_inserts_when_absent() {
    let crontab = "0 * * * * /existing\n";
    let block = "# BEGIN TEST\n# hdr\n# END TEST";
    let result = replace_block_with(crontab, block, TEST_BEGIN, TEST_END);
    assert!(result.contains(TEST_BEGIN));
    assert!(result.contains(TEST_END));
    assert!(result.contains("/existing"));
}

#[test]
fn replace_block_with_replaces_existing() {
    let crontab = "before\n# BEGIN TEST\nold line # tag:old\n# END TEST\nafter\n";
    let block = "# BEGIN TEST\nnew line # tag:new\n# END TEST";
    let result = replace_block_with(crontab, block, TEST_BEGIN, TEST_END);
    assert!(result.contains("new line"), "new line missing: {result}");
    assert!(
        !result.contains("old line"),
        "old line still present: {result}"
    );
    assert!(result.contains("before"), "before missing: {result}");
    assert!(result.contains("after"), "after missing: {result}");
}

#[test]
fn replace_block_with_idempotent() {
    let block = "# BEGIN TEST\n# hdr\n30 9 * * * /cmd # tag:uid\n# END TEST";
    let crontab = format!("{block}\n");
    let result = replace_block_with(&crontab, block, TEST_BEGIN, TEST_END);
    assert!(result.contains("30 9 * * * /cmd # tag:uid"));
}

#[test]
fn replace_block_with_handles_malformed_missing_end() {
    let crontab = "pre\n# BEGIN TEST\norphan line\n";
    let block = "# BEGIN TEST\n# hdr\n# END TEST";
    let result = replace_block_with(crontab, block, TEST_BEGIN, TEST_END);
    assert!(result.contains(TEST_END), "end marker missing: {result}");
    assert!(
        !result.contains("orphan"),
        "orphan line still present: {result}"
    );
    assert!(result.contains("pre"), "pre-content missing: {result}");
}

#[test]
fn replace_block_with_empty_crontab() {
    let block = "# BEGIN TEST\n# hdr\n# END TEST";
    let result = replace_block_with("", block, TEST_BEGIN, TEST_END);
    assert_eq!(result.trim(), block.trim());
}

#[test]
fn replace_block_with_appends_trailing_newline_to_unterminated_rest() {
    // Covers the `if !result.ends_with('\n')` branch: content follows the END
    // marker but does not end in a newline, so one is appended to preserve it.
    let crontab = "# BEGIN TEST\nold # tag:x\n# END TEST\ntrailing line no newline";
    let block = "# BEGIN TEST\nnew # tag:y\n# END TEST";
    let result = replace_block_with(crontab, block, TEST_BEGIN, TEST_END);
    assert!(
        result.contains("new # tag:y"),
        "block not replaced: {result}"
    );
    assert!(
        result.contains("trailing line no newline"),
        "trailing content lost: {result}"
    );
    assert!(
        result.ends_with('\n'),
        "trailing newline not appended: {result:?}"
    );
}

// ─── marker collision guard (issue #324) ─────────────────────────────────────

#[test]
fn replace_block_does_not_match_a_marker_as_a_prefix_of_another() {
    // A crontab holding only a block whose begin marker is a longer string that
    // has TEST_BEGIN as a prefix. A substring `find` would incorrectly match
    // inside it; whole-line matching must leave it untouched and append instead.
    let crontab = "# BEGIN TEST-LONGER\nunrelated # tag:rid\n# END TEST-LONGER\n";
    let block = "# BEGIN TEST\n# hdr\n30 9 * * * /cmd # tag:uid\n# END TEST";
    let result = replace_block_with(crontab, block, TEST_BEGIN, TEST_END);

    assert!(
        result.contains("# tag:rid"),
        "unrelated block was wiped: {result}"
    );
    assert!(
        result.contains("# BEGIN TEST-LONGER"),
        "unrelated begin marker lost: {result}"
    );
    assert!(
        result.contains("30 9 * * * /cmd # tag:uid"),
        "new block not appended: {result}"
    );
}

#[test]
fn replace_block_targets_exact_marker_among_similarly_named_blocks() {
    // Both a TEST block and a longer-named lookalike are present. Replacing the
    // TEST block must edit only it and leave the other block byte-for-byte intact.
    let crontab = "# BEGIN TEST\nold # tag:old\n# END TEST\n\
                   # BEGIN TEST-LONGER\nunrelated # tag:rid\n# END TEST-LONGER\n";
    let block = "# BEGIN TEST\nnew # tag:new\n# END TEST";
    let result = replace_block_with(crontab, block, TEST_BEGIN, TEST_END);

    assert!(result.contains("new # tag:new"), "not replaced: {result}");
    assert!(!result.contains("old # tag:old"), "stale line kept: {result}");
    assert!(
        result.contains("unrelated # tag:rid"),
        "lookalike block disturbed: {result}"
    );
    assert!(
        result.contains("# END TEST-LONGER"),
        "lookalike end marker lost: {result}"
    );
}

#[test]
fn find_marker_line_ignores_surrounding_whitespace() {
    // A marker indented with leading/trailing whitespace still matches by line,
    // and the reported offsets bracket the marker text exactly.
    let crontab = "noise\n  # END TEST  \n";
    let (start, end) = find_marker_line(crontab, "# END TEST").expect("marker found");
    assert_eq!(&crontab[start..end], "  # END TEST");
    assert!(find_marker_line(crontab, "# BEGIN TEST").is_none());
}
