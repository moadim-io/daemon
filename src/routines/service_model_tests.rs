#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use super::*;

use crate::routines::{new_store, slugify};

/// Build a routine with overridable identity, title, timestamps, and repository URL.
fn make_routine(id: &str, title: &str, created_at: u64, updated_at: u64) -> Routine {
    Routine {
        model: None,
        id: id.to_string(),
        schedule: "@daily".to_string(),
        schedules: vec![],
        title: title.to_string(),
        agent: "claude".to_string(),
        prompt: "do the thing".to_string(),
        goal: None,
        repositories: vec![],
        machines: vec![crate::machine::current_machine()],
        enabled: true,
        source: "managed".to_string(),
        created_at,
        updated_at,
        last_manual_trigger_at: None,
        last_scheduled_trigger_at: None,
        snoozed_until: None,
        skip_runs: None,
        power_saving: false,
        tags: vec![],
        ttl_secs: None,
        max_runtime_secs: None,
        env: std::collections::HashMap::new(),
        auto_disabled_reason: None,
        consecutive_failures: 0,
        failure_threshold: None,
    }
}

fn valid_create_request() -> CreateRoutineRequest {
    CreateRoutineRequest {
        model: None,
        schedule: "@daily".into(),
        schedules: vec![],
        title: "Valid Title".into(),
        agent: "claude".into(),
        prompt: "do the thing".into(),
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

/// Build a no-op update request (every field `None`); callers set one field.
fn empty_update_request() -> UpdateRoutineRequest {
    UpdateRoutineRequest {
        model: None,
        schedule: None,
        schedules: None,
        title: None,
        agent: None,
        prompt: None,
        goal: None,
        repositories: None,
        machines: None,
        enabled: None,
        ttl_secs: None,
        max_runtime_secs: None,
        tags: None,
        env: None,
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
include!("service_model_tests_part3.rs");
