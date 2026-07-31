#![allow(
    clippy::wildcard_imports,
    clippy::too_many_arguments,
    clippy::needless_pass_by_value,
    dead_code,
    reason = "split-out module preserves existing code while keeping files under the linecheck limit"
)]
use super::*;

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
