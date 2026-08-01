
#[test]
fn svc_create_rejects_duplicate_slug() {
    let _home = TempHome::set();
    // Covers the slug-conflict branch in `svc_create`: an existing routine whose
    // title slugifies to the same value forces a `Conflict`.
    let title = "Svc Create Dup ZZZ";
    let store = new_store();
    // `with_empty_path` so the post-create/delete crontab sync cannot spawn the
    // real `crontab` binary and clobber the developer's live crontab (issue #175).
    with_empty_path(|| {
        let first = svc_create(
            &store,
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
                power_saving_exempt: false,
                tags: vec![],
                env: std::collections::HashMap::new(),
                failure_threshold: None,
        notifications: Default::default(),
            },
        )
        .unwrap();

        let conflict = svc_create(
            &store,
            CreateRoutineRequest {
                model: None,
                schedule: "@daily".into(),
                schedules: vec![],
                // Different casing/spacing, same slug.
                title: "  svc create   DUP zzz ".into(),
                agent: "claude".into(),
                prompt: "p".into(),
                goal: None,
                repositories: vec![],
                machines: vec![crate::machine::current_machine()],
                enabled: true,
                ttl_secs: None,
                max_runtime_secs: None,
                power_saving_exempt: false,
                tags: vec![],
                env: std::collections::HashMap::new(),
                failure_threshold: None,
        notifications: Default::default(),
            },
        );
        assert!(matches!(conflict, Err(AppError::Conflict(_))));

        svc_delete(&store, &first.routine.id).unwrap();
    });
}

#[test]
fn svc_create_trims_title_before_persisting() {
    // Covers the title `.trim()` on the `svc_create` store path: a padded title is
    // length-checked trimmed but must also be *stored* trimmed, so the disclosure /
    // iCal SUMMARY / UI rows never render the surrounding whitespace.
    let title = "Svc Create Trim ZZZ";
    let store = new_store();
    with_empty_path(|| {
        let created = svc_create(
            &store,
            CreateRoutineRequest {
                title: "   Svc Create Trim ZZZ   ".into(),
                ..valid_create_request()
            },
        )
        .unwrap();
        assert_eq!(created.routine.title, title);
        svc_delete(&store, &created.routine.id).unwrap();
    });
    let _ = crate::routine_storage::remove_routine_dir(&slugify(title));
}

#[allow(
    clippy::missing_docs_in_private_items,
    reason = "split-out module keeps the file under the linecheck limit"
)]
#[path = "svc_create_rejects_malformed_agent_config_tests.rs"]
mod svc_create_rejects_malformed_agent_config_tests;
