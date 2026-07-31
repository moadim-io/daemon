
#[test]
fn svc_create_trims_and_persists_goal() {
    let _home = TempHome::set();
    // A present goal is trimmed and stored, and it survives a reload from disk.
    let title = "Svc Create Goal ZZZ";
    let store = new_store();
    with_empty_path(|| {
        let created = svc_create(
            &store,
            CreateRoutineRequest {
                schedule: "@daily".into(),
                schedules: vec![],
                title: title.into(),
                agent: "claude".into(),
                model: None,
                prompt: "p".into(),
                goal: Some("  keep the backlog small  ".into()),
                repositories: vec![],
                machines: vec![crate::machine::current_machine()],
                enabled: true,
                ttl_secs: None,
                max_runtime_secs: None,
                tags: vec![],
                env: std::collections::HashMap::new(),
                failure_threshold: None,
            },
        )
        .unwrap();
        assert_eq!(
            created.routine.goal.as_deref(),
            Some("keep the backlog small")
        );
        // Reloading the store from disk yields the same goal (persisted to routine.toml).
        let reloaded = crate::routine_storage::load_store();
        let stored = reloaded
            .lock()
            .unwrap()
            .get(&created.routine.id)
            .cloned()
            .expect("routine persisted");
        assert_eq!(stored.goal.as_deref(), Some("keep the backlog small"));
    });
}

#[test]
fn svc_update_clears_goal_with_empty_string() {
    let _home = TempHome::set();
    // `Some("")` on update clears the goal; `None` would instead keep the existing value.
    let title = "Svc Update Clear Goal ZZZ";
    let store = new_store();
    let mut routine = make_routine("upd-goal-id", title, 1, 1);
    routine.goal = Some("old goal".into());
    crate::routine_storage::write_routine(&routine).unwrap();
    store.lock().unwrap().insert("upd-goal-id".into(), routine);
    with_empty_path(|| {
        let updated = svc_update(
            &store,
            "upd-goal-id",
            UpdateRoutineRequest {
                schedule: None,
                schedules: None,
                title: None,
                agent: None,
                model: None,
                prompt: None,
                goal: Some(String::new()),
                repositories: None,
                machines: None,
                enabled: None,
                ttl_secs: None,
                max_runtime_secs: None,
                tags: None,
                env: None,
                failure_threshold: None,
            },
        )
        .unwrap();
        assert_eq!(updated.routine.goal, None);
    });
}

#[test]
fn svc_update_warns_when_crontab_sync_fails() {
    let _home = TempHome::set();
    // Same crontab-spawn failure as above, on the update path.
    let title = "Svc Update Sync Fail ZZZ";
    let store = new_store();
    let routine = make_routine("upd-sync-id", title, 1, 1);
    crate::routine_storage::write_routine(&routine).unwrap();
    store.lock().unwrap().insert("upd-sync-id".into(), routine);
    with_empty_path(|| {
        let updated = svc_update(
            &store,
            "upd-sync-id",
            UpdateRoutineRequest {
                model: None,
                schedule: None,
                schedules: None,
                title: None,
                agent: None,
                prompt: Some("changed".into()),
                goal: None,
                repositories: None,
                machines: None,
                enabled: None,
                ttl_secs: None,
                max_runtime_secs: None,
                tags: None,
                env: None,
                failure_threshold: None,
            },
        )
        .unwrap();
        assert_eq!(updated.routine.prompt, "changed");
    });
}

#[test]
fn svc_delete_warns_when_crontab_sync_fails() {
    let _home = TempHome::set();
    // Same crontab-spawn failure, on the delete path.
    let title = "Svc Delete Sync Fail ZZZ";
    let store = new_store();
    let routine = make_routine("del-sync-id", title, 1, 1);
    crate::routine_storage::write_routine(&routine).unwrap();
    store.lock().unwrap().insert("del-sync-id".into(), routine);
    with_empty_path(|| {
        let deleted = svc_delete(&store, "del-sync-id").unwrap();
        assert_eq!(deleted.routine.title, title);
    });
}
