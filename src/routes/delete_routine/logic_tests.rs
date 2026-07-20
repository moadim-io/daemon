#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use super::build;

#[test]
fn build_returns_not_found_for_unknown_id() {
    let store = crate::routines::new_store();
    let result = build(&store, "no-such");
    assert!(result.is_err());
}
