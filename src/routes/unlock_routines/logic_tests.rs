#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use super::build;

#[test]
fn build_rejects_unknown_scope() {
    let store = crate::routines::new_store();
    let result = build(&store, "no-such-scope");
    assert!(matches!(result, Err(crate::error::AppError::BadRequest(_))));
}
