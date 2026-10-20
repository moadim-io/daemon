#[test]
fn svc_update_sets_timezone_override() {
    let _home = TempHome::set();
    let title = "Svc Update Timezone Set ZZZ";
    let store = new_store();
    let routine = make_routine("tz-set-id", title, 1, 1);
    crate::routine_storage::write_routine(&routine).unwrap();
    store.lock().unwrap().insert("tz-set-id".into(), routine);

    with_empty_path(|| {
        let updated = svc_update(
            &store,
            "tz-set-id",
            UpdateRoutineRequest {
                timezone: Some("Asia/Jerusalem".to_string()),
                ..empty_update_request()
            },
        )
        .unwrap();
        assert_eq!(updated.routine.timezone.as_deref(), Some("Asia/Jerusalem"));
    });
}

#[test]
fn svc_update_clears_an_existing_timezone_override_via_blank_string() {
    let _home = TempHome::set();
    // Mirrors `model`'s clear-on-blank convention.
    let title = "Svc Update Timezone Clear ZZZ";
    let store = new_store();
    let mut routine = make_routine("tz-clear-id", title, 1, 1);
    routine.timezone = Some("Asia/Jerusalem".to_string());
    crate::routine_storage::write_routine(&routine).unwrap();
    store.lock().unwrap().insert("tz-clear-id".into(), routine);

    with_empty_path(|| {
        let updated = svc_update(
            &store,
            "tz-clear-id",
            UpdateRoutineRequest {
                timezone: Some("   ".to_string()),
                ..empty_update_request()
            },
        )
        .unwrap();
        assert_eq!(updated.routine.timezone, None);
    });
}

#[test]
fn svc_update_leaves_timezone_unchanged_when_the_field_is_omitted() {
    let _home = TempHome::set();
    let title = "Svc Update Timezone Untouched ZZZ";
    let store = new_store();
    let mut routine = make_routine("tz-untouched-id", title, 1, 1);
    routine.timezone = Some("Asia/Jerusalem".to_string());
    crate::routine_storage::write_routine(&routine).unwrap();
    store.lock().unwrap().insert("tz-untouched-id".into(), routine);

    with_empty_path(|| {
        let updated = svc_update(
            &store,
            "tz-untouched-id",
            UpdateRoutineRequest {
                ttl_secs: Some(1800),
                ..empty_update_request()
            },
        )
        .unwrap();
        assert_eq!(updated.routine.timezone.as_deref(), Some("Asia/Jerusalem"));
    });
}
