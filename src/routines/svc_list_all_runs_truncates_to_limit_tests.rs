
#[test]
fn svc_list_all_runs_truncates_to_limit() {
    let _home = TempHome::set();
    let title = "Fleet Runs Limit ZZQ";
    let slug = slugify(title);
    let store = new_store();
    store
        .lock()
        .unwrap()
        .insert("id".into(), make_routine("id", title));

    let workbenches = crate::paths::workbenches_dir();
    for ts in [1000, 2000, 3000] {
        std::fs::create_dir_all(workbenches.join(format!("{slug}-{ts}"))).unwrap();
    }

    let runs = svc_list_all_runs(&store, Some(2));
    assert_eq!(runs.len(), 2);
    assert_eq!(runs[0].started_at, 3000);
    assert_eq!(runs[1].started_at, 2000);
}

#[test]
fn svc_list_all_runs_empty_when_workbenches_dir_absent() {
    let _home = TempHome::set();
    assert_eq!(svc_list_all_runs(&new_store(), None), vec![]);
}

#[test]
fn svc_list_runs_merges_persisted_history_with_live_workbenches() {
    use crate::routines::run_history::{append_persisted_run, PersistedRun};

    let _home = TempHome::set();
    let title = "Runs Persisted ZZQ";
    let store = new_store();
    store
        .lock()
        .unwrap()
        .insert("id".into(), make_routine("id", title));

    let slug = slugify(title);
    std::fs::create_dir_all(crate::paths::workbenches_dir().join(format!("{slug}-3000"))).unwrap();
    append_persisted_run(
        "id",
        &PersistedRun {
            workbench: format!("{slug}-1000"),
            started_at: 1000,
            finished_at: 1005,
            status: RunStatus::Success,
            exit_code: Some(0),
        },
    );

    let runs = svc_list_runs(&store, "id").unwrap();
    assert_eq!(
        runs.iter().map(|run| run.started_at).collect::<Vec<_>>(),
        vec![3000, 1000],
        "the live workbench and the persisted (already-reaped) run both appear, newest first"
    );
    assert_eq!(runs[1].status, RunStatus::Success);
    assert_eq!(runs[1].finished_at, Some(1005));
}
