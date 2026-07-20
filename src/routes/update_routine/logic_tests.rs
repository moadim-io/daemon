#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use super::{build, UpdateRoutineRequest};

fn make_update_req() -> UpdateRoutineRequest {
    UpdateRoutineRequest {
        schedule: None,
        title: None,
        agent: None,
        model: None,
        prompt: None,
        goal: None,
        repositories: None,
        machines: None,
        enabled: None,
        ttl_secs: None,
        max_runtime_secs: None,
        tags: None,
        env: None,
    }
}

#[test]
fn build_returns_not_found_for_unknown_id() {
    let store = crate::routines::new_store();
    let result = build(&store, "no-such", make_update_req());
    assert!(result.is_err());
}
