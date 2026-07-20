#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use super::*;
use crate::routines::model::{new_store, Repository, Routine};

fn make_routine(id: &str, repositories: Vec<Repository>) -> Routine {
    Routine {
        model: None,
        id: id.to_string(),
        schedule: "@daily".to_string(),
        title: "My Routine".to_string(),
        agent: "claude".to_string(),
        prompt: "do the thing".to_string(),
        goal: None,
        repositories,
        machines: vec![crate::machine::current_machine()],
        enabled: true,
        source: "managed".to_string(),
        created_at: 0,
        updated_at: 0,
        last_manual_trigger_at: None,
        last_scheduled_trigger_at: None,
        snoozed_until: None,
        skip_runs: None,
        power_saving: false,
        tags: vec![],
        ttl_secs: None,
        max_runtime_secs: None,
        env: std::collections::HashMap::new(),
    }
}

#[test]
fn preview_matches_compose_prompt_with_no_repositories() {
    let routine = make_routine("no-repos", vec![]);
    let store = new_store();
    store
        .lock_recover()
        .insert(routine.id.clone(), routine.clone());

    let preview = svc_get_prompt_preview(&store, "no-repos").expect("routine exists");
    assert_eq!(preview, compose_prompt(&routine));
    assert!(preview.contains("You are working in an empty directory.\n"));
}

#[test]
fn preview_matches_compose_prompt_with_repositories() {
    let routine = make_routine(
        "with-repos",
        vec![
            Repository {
                repository: "https://github.com/octocat/Hello-World".to_string(),
                branch: None,
            },
            Repository {
                repository: "https://github.com/octocat/Spoon-Knife".to_string(),
                branch: Some("main".to_string()),
            },
        ],
    );
    let store = new_store();
    store
        .lock_recover()
        .insert(routine.id.clone(), routine.clone());

    let preview = svc_get_prompt_preview(&store, "with-repos").expect("routine exists");
    assert_eq!(preview, compose_prompt(&routine));
    assert!(preview.contains("- https://github.com/octocat/Hello-World\n"));
    assert!(preview.contains("- https://github.com/octocat/Spoon-Knife (branch main)\n"));
}

#[test]
fn preview_not_found_for_unknown_id() {
    let store = new_store();
    assert!(svc_get_prompt_preview(&store, "missing").is_err());
}
