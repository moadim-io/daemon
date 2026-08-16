
#[test]
fn svc_update_sets_max_runtime_secs() {
    let _home = TempHome::set();
    // Covers the `req.max_runtime_secs` apply branch in `svc_update`.
    let title = "Svc Update Max Runtime ZZZ";
    let store = new_store();
    let routine = make_routine("max-runtime-id", title, 1, 1);
    crate::routine_storage::write_routine(&routine).unwrap();
    store
        .lock()
        .unwrap()
        .insert("max-runtime-id".into(), routine);

    // `with_empty_path` keeps the post-update crontab sync from touching the real
    // crontab (issue #175): the update succeeds, the sync just warns.
    with_empty_path(|| {
        let updated = svc_update(
            &store,
            "max-runtime-id",
            UpdateRoutineRequest {
        disabled_reason: None,
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
                max_runtime_secs: Some(1234),
                power_saving_exempt: None,
                tags: None,
                env: None,
                failure_threshold: None,
        notifications: Default::default(),
                timezone: None,
            },
        )
        .unwrap();
        assert_eq!(updated.routine.max_runtime_secs, Some(1234));
    });
}

#[test]
fn svc_update_sets_env() {
    let _home = TempHome::set();
    // Covers the `req.env` validate + apply branches in `svc_update` (#408).
    let title = "Svc Update Env ZZZ";
    let store = new_store();
    let routine = make_routine("env-id", title, 1, 1);
    crate::routine_storage::write_routine(&routine).unwrap();
    store.lock().unwrap().insert("env-id".into(), routine);

    with_empty_path(|| {
        let updated = svc_update(
            &store,
            "env-id",
            UpdateRoutineRequest {
        disabled_reason: None,
                env: Some(std::collections::HashMap::from([(
                    "MODEL_OVERRIDE".to_string(),
                    "gpt-x".to_string(),
                )])),
                ..empty_update_request()
            },
        )
        .unwrap();
        assert_eq!(
            updated
                .routine
                .env
                .get("MODEL_OVERRIDE")
                .map(String::as_str),
            Some("gpt-x")
        );
    });
}

#[test]
fn svc_update_sets_failure_threshold() {
    let _home = TempHome::set();
    // Covers the `req.failure_threshold` apply branch in `svc_update`.
    let title = "Svc Update Failure Threshold ZZZ";
    let store = new_store();
    let routine = make_routine("threshold-id", title, 1, 1);
    crate::routine_storage::write_routine(&routine).unwrap();
    store.lock().unwrap().insert("threshold-id".into(), routine);

    with_empty_path(|| {
        let updated = svc_update(
            &store,
            "threshold-id",
            UpdateRoutineRequest {
        disabled_reason: None,
                failure_threshold: Some(5),
        notifications: Default::default(),
                ..empty_update_request()
            },
        )
        .unwrap();
        assert_eq!(updated.routine.failure_threshold, Some(5));
    });
}

#[allow(
    clippy::missing_docs_in_private_items,
    reason = "split-out module keeps the file under the linecheck limit"
)]
#[path = "svc_update_re_enabling_resets_circuit_breaker_state_tests.rs"]
mod svc_update_re_enabling_resets_circuit_breaker_state_tests;
