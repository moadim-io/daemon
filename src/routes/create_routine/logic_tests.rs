#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use super::{build, CreateRoutineRequest};

fn make_req() -> CreateRoutineRequest {
    CreateRoutineRequest {
        model: None,
        schedule: "not-a-cron".into(),
        title: "Logic Create Routine".into(),
        agent: "claude".into(),
        prompt: "p".into(),
        goal: None,
        repositories: vec![],
        machines: vec![],
        enabled: true,
        ttl_secs: None,
        max_runtime_secs: None,
        tags: vec![],
        env: std::collections::HashMap::new(),
    }
}

#[test]
fn build_rejects_invalid_cron() {
    let store = crate::routines::new_store();
    let result = build(&store, make_req());
    assert!(result.is_err());
}
