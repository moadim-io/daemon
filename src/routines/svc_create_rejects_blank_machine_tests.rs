
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
            power_saving_exempt: false,
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

#[allow(
    clippy::missing_docs_in_private_items,
    reason = "split-out module keeps the file under the linecheck limit"
)]
#[path = "svc_update_rejects_and_sets_tags_tests.rs"]
mod svc_update_rejects_and_sets_tags_tests;
