#![allow(
    clippy::wildcard_imports,
    clippy::too_many_arguments,
    clippy::needless_pass_by_value,
    dead_code,
    reason = "split-out module preserves existing code while keeping files under the linecheck limit"
)]
use super::*;

#[test]
fn svc_run_log_returns_specific_workbench_log() {
    let _home = TempHome::set();
    let title = "Run Log Exact ZZQ";
    let slug = slugify(title);
    let store = new_store();
    store
        .lock()
        .unwrap()
        .insert("id".into(), make_routine("id", title));

    let workbenches = crate::paths::workbenches_dir();
    let older = format!("{slug}-1000");
    let newer = format!("{slug}-2000");
    std::fs::create_dir_all(workbenches.join(&older)).unwrap();
    std::fs::create_dir_all(workbenches.join(&newer)).unwrap();
    std::fs::write(workbenches.join(&older).join("agent.log"), "older run").unwrap();
    std::fs::write(workbenches.join(&newer).join("agent.log"), "newer run").unwrap();

    // Explicitly asking for the *older* run's log must not fall back to the newest, unlike
    // `svc_logs`.
    assert_eq!(svc_run_log(&store, "id", &older).unwrap(), "older run");
    assert_eq!(svc_run_log(&store, "id", &newer).unwrap(), "newer run");
}

#[test]
fn svc_run_summary_missing_routine_not_found() {
    let _home = TempHome::set();
    assert!(matches!(
        svc_run_summary(&new_store(), "nope", "whatever-1"),
        Err(AppError::NotFound)
    ));
}

#[test]
fn svc_run_summary_not_found_for_unparseable_workbench_name() {
    let _home = TempHome::set();
    let title = "Run Summary Bad Name ZZQ";
    let store = new_store();
    store
        .lock()
        .unwrap()
        .insert("id".into(), make_routine("id", title));

    assert!(matches!(
        svc_run_summary(&store, "id", "not-a-workbench"),
        Err(AppError::NotFound)
    ));
}

#[test]
fn svc_run_summary_not_found_for_foreign_workbench() {
    let _home = TempHome::set();
    let title = "Run Summary Foreign ZZQ";
    let store = new_store();
    store
        .lock()
        .unwrap()
        .insert("id".into(), make_routine("id", title));

    assert!(matches!(
        svc_run_summary(&store, "id", "some-other-routine-9999"),
        Err(AppError::NotFound)
    ));
}

#[test]
fn svc_run_summary_empty_when_summary_missing() {
    let _home = TempHome::set();
    let title = "Run Summary Missing File ZZQ";
    let slug = slugify(title);
    let store = new_store();
    store
        .lock()
        .unwrap()
        .insert("id".into(), make_routine("id", title));

    let workbench = format!("{slug}-1000");
    std::fs::create_dir_all(crate::paths::workbenches_dir().join(&workbench)).unwrap();

    assert_eq!(svc_run_summary(&store, "id", &workbench).unwrap(), "");
}

#[test]
fn svc_run_summary_returns_specific_workbench_summary() {
    let _home = TempHome::set();
    let title = "Run Summary Exact ZZQ";
    let slug = slugify(title);
    let store = new_store();
    store
        .lock()
        .unwrap()
        .insert("id".into(), make_routine("id", title));

    let workbenches = crate::paths::workbenches_dir();
    let older = format!("{slug}-1000");
    let newer = format!("{slug}-2000");
    std::fs::create_dir_all(workbenches.join(&older)).unwrap();
    std::fs::create_dir_all(workbenches.join(&newer)).unwrap();
    std::fs::write(workbenches.join(&older).join("summary.md"), "older summary").unwrap();
    std::fs::write(workbenches.join(&newer).join("summary.md"), "newer summary").unwrap();

    assert_eq!(
        svc_run_summary(&store, "id", &older).unwrap(),
        "older summary"
    );
    assert_eq!(
        svc_run_summary(&store, "id", &newer).unwrap(),
        "newer summary"
    );
}

#[test]
fn svc_list_all_runs_merges_across_routines_newest_first() {
    let _home = TempHome::set();
    let title_a = "Fleet Runs A ZZQ";
    let title_b = "Fleet Runs B ZZQ";
    let slug_a = slugify(title_a);
    let slug_b = slugify(title_b);
    let store = new_store();
    store
        .lock()
        .unwrap()
        .insert("a-id".into(), make_routine("a-id", title_a));
    store
        .lock()
        .unwrap()
        .insert("b-id".into(), make_routine("b-id", title_b));

    let workbenches = crate::paths::workbenches_dir();
    std::fs::create_dir_all(workbenches.join(format!("{slug_a}-1000"))).unwrap();
    std::fs::create_dir_all(workbenches.join(format!("{slug_b}-3000"))).unwrap();
    std::fs::create_dir_all(workbenches.join(format!("{slug_a}-2000"))).unwrap();
    // A workbench with no matching routine (deleted since) must not appear.
    std::fs::create_dir_all(workbenches.join("some-deleted-routine-9999")).unwrap();
    // Not a `{slug}-{ts}` directory at all: parse_workbench_name returns None.
    std::fs::create_dir_all(workbenches.join("not-a-workbench-name")).unwrap();

    let runs = svc_list_all_runs(&store, None);
    assert_eq!(
        runs.iter().map(|run| run.started_at).collect::<Vec<_>>(),
        vec![3000, 2000, 1000]
    );
    assert_eq!(runs[0].routine_id, "b-id");
    assert_eq!(runs[0].routine_title, title_b);
    assert_eq!(runs[1].routine_id, "a-id");
    assert_eq!(runs[2].routine_id, "a-id");
}

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
