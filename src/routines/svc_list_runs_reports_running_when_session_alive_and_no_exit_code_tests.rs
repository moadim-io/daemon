
#[test]
fn svc_list_runs_reports_running_when_session_alive_and_no_exit_code() {
    let _home = TempHome::set();
    let title = "Runs Running ZZQ";
    let slug = slugify(title);
    let store = new_store();
    store
        .lock()
        .unwrap()
        .insert("id".into(), make_routine("id", title));

    let workbenches = crate::paths::workbenches_dir();
    std::fs::create_dir_all(workbenches.join(format!("{slug}-1000"))).unwrap();

    with_tmux_alive(|| {
        let runs = svc_list_runs(&store, "id").unwrap();
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].status, RunStatus::Running);
        assert_eq!(runs[0].exit_code, None);
    });
}

#[test]
fn svc_run_log_missing_routine_not_found() {
    let _home = TempHome::set();
    assert!(matches!(
        svc_run_log(&new_store(), "nope", "whatever-1"),
        Err(AppError::NotFound)
    ));
}

#[test]
fn svc_run_log_not_found_for_unparseable_workbench_name() {
    let _home = TempHome::set();
    let title = "Run Log Bad Name ZZQ";
    let store = new_store();
    store
        .lock()
        .unwrap()
        .insert("id".into(), make_routine("id", title));

    assert!(matches!(
        svc_run_log(&store, "id", "not-a-workbench"),
        Err(AppError::NotFound)
    ));
}

#[test]
fn svc_run_log_not_found_for_foreign_workbench() {
    let _home = TempHome::set();
    let title = "Run Log Foreign ZZQ";
    let store = new_store();
    store
        .lock()
        .unwrap()
        .insert("id".into(), make_routine("id", title));

    assert!(matches!(
        svc_run_log(&store, "id", "some-other-routine-9999"),
        Err(AppError::NotFound)
    ));
}

#[test]
fn svc_run_log_empty_when_agent_log_missing() {
    let _home = TempHome::set();
    let title = "Run Log Missing File ZZQ";
    let slug = slugify(title);
    let store = new_store();
    store
        .lock()
        .unwrap()
        .insert("id".into(), make_routine("id", title));

    let workbench = format!("{slug}-1000");
    std::fs::create_dir_all(crate::paths::workbenches_dir().join(&workbench)).unwrap();

    assert_eq!(svc_run_log(&store, "id", &workbench).unwrap(), "");
}

#[allow(
    clippy::missing_docs_in_private_items,
    reason = "split-out module keeps the file under the linecheck limit"
)]
#[path = "svc_run_log_returns_specific_workbench_log_tests.rs"]
mod svc_run_log_returns_specific_workbench_log_tests;

#[allow(
    clippy::missing_docs_in_private_items,
    reason = "split-out module keeps the file under the linecheck limit"
)]
#[path = "svc_list_all_runs_merges_persisted_history_across_routines_tests.rs"]
mod svc_list_all_runs_merges_persisted_history_across_routines_tests;
