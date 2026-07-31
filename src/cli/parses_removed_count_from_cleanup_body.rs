#![allow(
    clippy::wildcard_imports,
    clippy::too_many_arguments,
    clippy::needless_pass_by_value,
    dead_code,
    reason = "split-out module preserves existing code while keeping files under the linecheck limit"
)]
use super::*;

#[test]
fn parses_removed_count_from_cleanup_body() {
    assert_eq!(parse_removed_count("{\"removed\":0}"), Some(0));
    assert_eq!(parse_removed_count("{\"removed\":7}"), Some(7));
}

#[test]
fn rejects_non_cleanup_body() {
    assert_eq!(parse_removed_count(""), None);
    assert_eq!(parse_removed_count("not json"), None);
    assert_eq!(parse_removed_count("{\"other\":1}"), None);
}
