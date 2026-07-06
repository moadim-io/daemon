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
