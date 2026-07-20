#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use super::build;

#[test]
fn build_rejects_bad_scope() {
    let store = crate::routines::new_store();
    let result = build(&store, "no-such", "bug", "d", "nowhere");
    assert!(result.is_err());
}

#[test]
fn build_not_found_is_error() {
    let store = crate::routines::new_store();
    let result = build(&store, "no-such", "bug", "d", "general");
    assert!(result.is_err());
}
