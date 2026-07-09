#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use super::*;

#[test]
fn print_does_not_panic() {
    print("127.0.0.1:5784");
}
