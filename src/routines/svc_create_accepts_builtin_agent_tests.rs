
#[test]
fn svc_create_accepts_builtin_agent() {
    let _home = TempHome::set();
    // Covers the success path of agent validation: a built-in agent
    // (`ensure_default_agents` seeds `claude`/`codex`) is accepted.
    crate::routines::ensure_default_agents();
    let title = "Svc Create Valid Agent ZZZ";
    let store = new_store();
    let created = svc_create(
        &store,
        CreateRoutineRequest {
        disabled_reason: None,
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
    assert_eq!(created.routine.agent, "claude");

    svc_delete(&store, &created.routine.id).unwrap();
}

#[test]
fn svc_create_rejects_blank_repository_url() {
    let _home = TempHome::set();
    // Covers the repositories-validation branch in `svc_create` (#241): an entry
    // whose URL is empty or whitespace-only must fail loud with `BadRequest`
    // instead of being stored and rendered as a broken `- ` clone bullet.
    let store = new_store();
    for url in ["", "   "] {
        let result = svc_create(
            &store,
            CreateRoutineRequest {
        disabled_reason: None,
                model: None,
                schedule: "@daily".into(),
                schedules: vec![],
                title: "Svc Create Blank Repo ZZZ".into(),
                agent: "claude".into(),
                prompt: "p".into(),
                goal: None,
                repositories: vec![Repository {
                    repository: url.into(),
                    branch: None,
                    auto_pull: true,
                }],
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
        assert!(matches!(result, Err(AppError::BadRequest(_))));
    }
    // Nothing should have been persisted.
    assert!(store.lock().unwrap().is_empty());
}

#[test]
fn svc_create_rejects_blank_repository_branch() {
    let _home = TempHome::set();
    // Covers the optional-branch guard: a `Some` branch that is empty/whitespace
    // must be rejected so `compose_prompt` cannot emit `- url (branch )`.
    let store = new_store();
    let result = svc_create(
        &store,
        CreateRoutineRequest {
        disabled_reason: None,
            model: None,
            schedule: "@daily".into(),
            schedules: vec![],
            title: "Svc Create Blank Branch ZZZ".into(),
            agent: "claude".into(),
            prompt: "p".into(),
            goal: None,
            repositories: vec![Repository {
                repository: "https://github.com/octocat/Hello-World".into(),
                branch: Some("  ".into()),
                auto_pull: true,
            }],
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
    assert!(matches!(result, Err(AppError::BadRequest(_))));
    assert!(store.lock().unwrap().is_empty());
}

#[allow(
    clippy::missing_docs_in_private_items,
    reason = "split-out module keeps the file under the linecheck limit"
)]
#[path = "svc_create_trims_repository_entries_tests.rs"]
mod svc_create_trims_repository_entries_tests;
