
#[cfg(unix)]
#[test]
fn svc_create_returns_internal_on_write_failure() {
    use std::os::unix::fs::PermissionsExt as _;
    // Covers L304: `write_routine(..).map_err(|_| AppError::Internal)?` in `svc_create`.
    // The slug dir is pre-created, then made read-only so the atomic write of
    // `routine.toml` fails.
    let _home = TempHome::set();
    let title = "Svc Create Write Fail ZZZ";
    let slug = slugify(title);
    let dir = crate::paths::routine_dir(&slug);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o555)).unwrap();

    let store = new_store();
    let result = svc_create(
        &store,
        CreateRoutineRequest {
        disabled_reason: None,
            model: None,
            title: title.into(),
            ..valid_create_request()
        },
    );

    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert!(matches!(result, Err(AppError::Internal)));
    // Nothing should have been inserted into the store.
    assert!(store.lock().unwrap().is_empty());
}

#[test]
fn svc_create_rejects_blank_tag() {
    // Covers the tags-validation error branch in `svc_create`: a blank or
    // whitespace-only tag must 400 before anything is persisted. `ensure_default_agents`
    // makes the agent check pass so validation reaches `validate_tags`.
    crate::routines::ensure_default_agents();
    let store = new_store();
    for tag in ["", "   "] {
        let result = svc_create(
            &store,
            CreateRoutineRequest {
        disabled_reason: None,
                model: None,
                power_saving_exempt: false,
                tags: vec![tag.to_string()],
                ..valid_create_request()
            },
        );
        assert!(matches!(result, Err(AppError::BadRequest(_))));
    }
    assert!(store.lock().unwrap().is_empty());
}

#[test]
fn svc_update_none_schedule_uses_existing_schedule() {
    let _home = TempHome::set();
    // Covers L359: the `None => lock.get(id)?.schedule.clone()` arm. When no new
    // schedule is supplied the ceiling check must derive from the stored schedule.
    let store = new_store();
    let routine = make_routine("upd-none-sched-id", "Upd None Sched ZZZ", 1, 1);
    crate::routine_storage::write_routine(&routine).unwrap();
    store
        .lock()
        .unwrap()
        .insert("upd-none-sched-id".into(), routine);
    with_empty_path(|| {
        let updated = svc_update(
            &store,
            "upd-none-sched-id",
            UpdateRoutineRequest {
        disabled_reason: None,
                model: None,
                prompt: Some("updated prompt".into()),
                goal: None,
                ..empty_update_request()
            },
        )
        .expect("update should succeed");
        assert_eq!(updated.routine.schedule, "@daily");
    });
}

#[allow(
    clippy::missing_docs_in_private_items,
    reason = "split-out module keeps the file under the linecheck limit"
)]
#[path = "svc_update_with_explicit_schedule_applies_it_tests.rs"]
mod svc_update_with_explicit_schedule_applies_it_tests;
