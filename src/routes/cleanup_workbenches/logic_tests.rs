#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use super::build;

#[test]
fn build_returns_zero_for_fresh_store() {
    let store = crate::routines::new_store();
    let resp = build(&store);
    assert_eq!(resp.removed, 0);
    assert_eq!(resp.freed_bytes, 0);
}
