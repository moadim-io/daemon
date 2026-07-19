#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use super::build;

#[test]
fn build_returns_non_empty_agent_list() {
    let agents = build();
    assert!(!agents.is_empty(), "agents list should never be empty");
}
