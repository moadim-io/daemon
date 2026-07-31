
#[test]
fn svc_list_sorts_by_repository_no_repo_last() {
    let dir = scratch_routines_dir();
    let mut zeta = make_routine("zeta");
    zeta.repositories = vec![Repository {
        repository: "https://github.com/octocat/Zeta".to_string(),
        branch: None,
    }];
    let mut apple = make_routine("apple");
    apple.repositories = vec![Repository {
        repository: "https://github.com/octocat/Apple".to_string(),
        branch: None,
    }];
    let mut none = make_routine("none");
    none.repositories = vec![];
    write_routine_to(&dir, &zeta);
    write_routine_to(&dir, &apple);
    write_routine_to(&dir, &none);
    let query = RoutineListQuery {
        sort: RoutineSort::Repository,
        ..Default::default()
    };
    let list = svc_list(&new_store(), &dir, &query);
    assert_eq!(list[0].routine.id, "apple");
    assert_eq!(list[1].routine.id, "zeta");
    // Routines with no repository sort last.
    assert_eq!(list[2].routine.id, "none");
}

#[test]
fn svc_get_reflects_routine_written_after_store_built() {
    // A routine written to disk *after* the (empty) store was built becomes visible on the next get
    // without rebuilding the store — the core "load the machines in every get" fix.
    let dir = scratch_routines_dir();
    let store = new_store();
    assert!(svc_get(&store, &dir, "appears").is_err());
    write_routine_to(&dir, &make_routine("appears"));
    assert_eq!(
        svc_get(&store, &dir, "appears").unwrap().routine.id,
        "appears"
    );
}

#[test]
fn svc_get_reflects_routine_removed_on_disk() {
    // A routine removed on disk disappears from the next get without a restart.
    let dir = scratch_routines_dir();
    write_routine_to(&dir, &make_routine("gone"));
    let store = new_store();
    assert!(svc_get(&store, &dir, "gone").is_ok());
    std::fs::remove_dir_all(dir.join("gone")).unwrap();
    assert!(svc_get(&store, &dir, "gone").is_err());
}

#[test]
fn svc_get_reflects_machines_edit_on_disk() {
    // Editing a routine's `machines` list on disk (e.g. via a `git pull`) is reflected on the next
    // get without a restart — the exact field the user reported as stale.
    let dir = scratch_routines_dir();
    let mut routine = make_routine("machines-edit");
    routine.machines = vec!["host-a".to_string()];
    write_routine_to(&dir, &routine);
    let store = new_store();
    assert_eq!(
        svc_get(&store, &dir, "machines-edit")
            .unwrap()
            .routine
            .machines,
        vec!["host-a".to_string()]
    );
    routine.machines = vec!["host-a".to_string(), "host-b".to_string()];
    write_routine_to(&dir, &routine);
    assert_eq!(
        svc_get(&store, &dir, "machines-edit")
            .unwrap()
            .routine
            .machines,
        vec!["host-a".to_string(), "host-b".to_string()]
    );
}

#[test]
fn reload_preserves_last_scheduled_trigger_at_sidecar() {
    // The reload goes through the same load path that reads the gitignored `scheduled.log`
    // append-only log, so the scheduler-written `last_scheduled_trigger_at` survives a reload
    // rather than being clobbered.
    let dir = scratch_routines_dir();
    write_routine_to(&dir, &make_routine("sched"));
    // The launch command appends to this log at each scheduled firing; the daemon only reads it.
    std::fs::write(dir.join("sched").join("scheduled.log"), "1717000000\n").unwrap();
    let store = new_store();
    let resp = svc_get(&store, &dir, "sched").unwrap();
    assert_eq!(resp.routine.last_scheduled_trigger_at, Some(1_717_000_000));
}
