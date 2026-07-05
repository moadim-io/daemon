#![allow(clippy::missing_docs_in_private_items)]

use super::*;

use crate::routines::{new_store, slugify};

/// Build a routine with overridable identity, title, timestamps, and repository URL.
fn make_routine(id: &str, title: &str, created_at: u64, updated_at: u64) -> Routine {
    Routine {
        model: None,
        id: id.to_string(),
        schedule: "@daily".to_string(),
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
        tags: vec![],
        ttl_secs: None,
        max_runtime_secs: None,
    }
}

fn valid_create_request() -> CreateRoutineRequest {
    CreateRoutineRequest {
        model: None,
        schedule: "@daily".into(),
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
    }
}

fn create_req_with_title(title: &str) -> CreateRoutineRequest {
    CreateRoutineRequest {
        model: None,
        schedule: "@daily".into(),
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
    }
}

/// Build a no-op update request (every field `None`); callers set one field.
fn empty_update_request() -> UpdateRoutineRequest {
    UpdateRoutineRequest {
        model: None,
        schedule: None,
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
fn svc_create_rejects_blank_machine() {
    // Covers the machines-validation error branch in `svc_create` (#600): an
    // empty or whitespace-only machines entry must 400 before anything is persisted,
    // rather than silently persisting an entry that can never match `machine::targets`.
    crate::routines::ensure_default_agents();
    let store = new_store();
    for machine in ["", "   "] {
        let result = svc_create(
            &store,
            CreateRoutineRequest {
                machines: vec![machine.to_string()],
                ..valid_create_request()
            },
        );
        assert!(matches!(result, Err(AppError::BadRequest(_))));
    }
    assert!(store.lock().unwrap().is_empty());
}

#[test]
fn svc_create_trims_and_dedupes_machines() {
    // Covers the normalize/Ok path of `validate_machines`: surrounding whitespace is
    // trimmed and a duplicate (post-trim) entry is collapsed to one (#600).
    crate::routines::ensure_default_agents();
    let title = "Svc Create Machines ZZZ";
    let store = new_store();
    let created = svc_create(
        &store,
        CreateRoutineRequest {
            machines: vec!["  laptop  ".into(), "laptop".into(), "server".into()],
            ..create_req_with_title(title)
        },
    )
    .unwrap();
    assert_eq!(
        created.routine.machines,
        vec!["laptop".to_string(), "server".to_string()]
    );

    svc_delete(&store, &created.routine.id).unwrap();
    let _ = crate::routine_storage::remove_routine_dir(&slugify(title));
}

#[test]
fn svc_update_rejects_and_sets_machines() {
    // Covers both the error and the apply arms of the `machines` handling in
    // `svc_update`: a blank entry is rejected, while a valid (trimmed, deduped)
    // list replaces the routine's machines (#600).
    let title = "Svc Update Machines ZZZ";
    let store = new_store();
    let routine = make_routine("upd-machines-id", title, 1, 1);
    crate::routine_storage::write_routine(&routine).unwrap();
    store
        .lock()
        .unwrap()
        .insert("upd-machines-id".into(), routine);

    let bad = svc_update(
        &store,
        "upd-machines-id",
        UpdateRoutineRequest {
            machines: Some(vec![" ".into()]),
            ..empty_update_request()
        },
    );
    assert!(matches!(bad, Err(AppError::BadRequest(_))));

    let updated = svc_update(
        &store,
        "upd-machines-id",
        UpdateRoutineRequest {
            machines: Some(vec!["  laptop  ".into(), "laptop".into()]),
            ..empty_update_request()
        },
    )
    .unwrap();
    assert_eq!(updated.routine.machines, vec!["laptop".to_string()]);

    let _ = crate::routine_storage::remove_routine_dir(&slugify(title));
}

#[test]
fn svc_update_rejects_and_sets_tags() {
    // Covers both the error and the apply arms of the `tags` handling in `svc_update`:
    // a blank tag is rejected, while a valid (trimmed) list replaces the routine's tags.
    let title = "Svc Update Tags ZZZ";
    let store = new_store();
    let routine = make_routine("upd-tags-id", title, 1, 1);
    crate::routine_storage::write_routine(&routine).unwrap();
    store.lock().unwrap().insert("upd-tags-id".into(), routine);

    let bad = svc_update(
        &store,
        "upd-tags-id",
        UpdateRoutineRequest {
            model: None,
            tags: Some(vec![" ".into()]),
            ..empty_update_request()
        },
    );
    assert!(matches!(bad, Err(AppError::BadRequest(_))));

    let updated = svc_update(
        &store,
        "upd-tags-id",
        UpdateRoutineRequest {
            model: None,
            tags: Some(vec!["  ops  ".into()]),
            ..empty_update_request()
        },
    )
    .unwrap();
    assert_eq!(updated.routine.tags, vec!["ops".to_string()]);

    let _ = crate::routine_storage::remove_routine_dir(&slugify(title));
}

#[test]
fn svc_create_trims_model_and_blank_normalizes_to_none() {
    // Covers both arms of `normalize_model` via `svc_create`: surrounding whitespace is
    // trimmed and stored, while a blank/whitespace-only value is stored as `None`.
    crate::routines::ensure_default_agents();
    let title = "Svc Create Model ZZZ";
    let store = new_store();
    let created = svc_create(
        &store,
        CreateRoutineRequest {
            model: Some("  claude-sonnet-4-6  ".into()),
            ..create_req_with_title(title)
        },
    )
    .unwrap();
    assert_eq!(created.routine.model, Some("claude-sonnet-4-6".to_string()));
    svc_delete(&store, &created.routine.id).unwrap();
    let _ = crate::routine_storage::remove_routine_dir(&slugify(title));

    let title2 = "Svc Create Blank Model ZZZ";
    let created2 = svc_create(
        &store,
        CreateRoutineRequest {
            model: Some("   ".into()),
            ..create_req_with_title(title2)
        },
    )
    .unwrap();
    assert_eq!(created2.routine.model, None);
    svc_delete(&store, &created2.routine.id).unwrap();
    let _ = crate::routine_storage::remove_routine_dir(&slugify(title2));
}

#[test]
fn svc_update_sets_and_clears_model() {
    // Covers the apply arm of the `model` handling in `svc_update`: a non-blank value is
    // trimmed and stored, and a subsequent blank value clears it back to `None`.
    let title = "Svc Update Model ZZZ";
    let store = new_store();
    let routine = make_routine("upd-model-id", title, 1, 1);
    crate::routine_storage::write_routine(&routine).unwrap();
    store.lock().unwrap().insert("upd-model-id".into(), routine);

    let updated = svc_update(
        &store,
        "upd-model-id",
        UpdateRoutineRequest {
            model: Some("  claude-opus-4-8  ".into()),
            ..empty_update_request()
        },
    )
    .unwrap();
    assert_eq!(updated.routine.model, Some("claude-opus-4-8".to_string()));

    let cleared = svc_update(
        &store,
        "upd-model-id",
        UpdateRoutineRequest {
            model: Some("  ".into()),
            ..empty_update_request()
        },
    )
    .unwrap();
    assert_eq!(cleared.routine.model, None);

    let _ = crate::routine_storage::remove_routine_dir(&slugify(title));
}
