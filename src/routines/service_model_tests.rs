#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use super::*;

use crate::routines::{new_store, slugify};

fn create_req_with_title(title: &str) -> CreateRoutineRequest {
    CreateRoutineRequest {
        model: None,
        schedule: "@daily".into(),
        schedules: vec![],
        title: title.into(),
        agent: "claude".into(),
        prompt: "p".into(),
        goal: None,
        repositories: vec![],
        machines: vec![crate::machine::current_machine()],
        enabled: true,
        ttl_secs: None,
        max_runtime_secs: None,
        tags: vec![],
        env: std::collections::HashMap::new(),
        failure_threshold: None,
    }
}

// ─── Tags / machines / model tests ───────────────────────────────────────────

#[test]
fn svc_create_trims_and_stores_tags() {
    // Covers the normalize/Ok path of `validate_tags` and the `tags` assignment in
    // `svc_create`: surrounding whitespace is trimmed and the tags are stored.
    crate::routines::ensure_default_agents();
    let title = "Svc Create Tags ZZZ";
    let store = new_store();
    let created = svc_create(
        &store,
        CreateRoutineRequest {
            model: None,
            tags: vec!["  triage  ".into(), "nightly".into()],
            ..create_req_with_title(title)
        },
    )
    .unwrap();
    assert_eq!(
        created.routine.tags,
        vec!["triage".to_string(), "nightly".to_string()]
    );

    svc_delete(&store, &created.routine.id).unwrap();
    let _ = crate::routine_storage::remove_routine_dir(&slugify(title));
}

#[test]
fn svc_create_dedupes_tags() {
    // Covers the dedup step of `validate_tags`: a duplicate (post-trim) tag entry is
    // collapsed to one, mirroring `validate_machines`'s dedup behavior.
    crate::routines::ensure_default_agents();
    let title = "Svc Create Tags Dedup ZZZ";
    let store = new_store();
    let created = svc_create(
        &store,
        CreateRoutineRequest {
            tags: vec!["  nightly  ".into(), "nightly".into(), "triage".into()],
            ..create_req_with_title(title)
        },
    )
    .unwrap();
    assert_eq!(
        created.routine.tags,
        vec!["nightly".to_string(), "triage".to_string()]
    );

    svc_delete(&store, &created.routine.id).unwrap();
    let _ = crate::routine_storage::remove_routine_dir(&slugify(title));
}
