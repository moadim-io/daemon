#![allow(
    clippy::wildcard_imports,
    clippy::too_many_arguments,
    clippy::needless_pass_by_value,
    dead_code,
    reason = "split-out module preserves existing code while keeping files under the linecheck limit"
)]
use super::*;

#[test]
fn svc_list_all_runs_merges_persisted_history_across_routines() {
    use crate::routines::run_history::{append_persisted_run, PersistedRun};

    let _home = TempHome::set();
    let title = "Fleet Runs Persisted ZZQ";
    let store = new_store();
    store
        .lock()
        .unwrap()
        .insert("id".into(), make_routine("id", title));

    append_persisted_run(
        "id",
        &PersistedRun {
            workbench: format!("{}-1000", slugify(title)),
            started_at: 1000,
            finished_at: 1005,
            status: RunStatus::Failed,
            exit_code: Some(1),
        },
    );
    // A persisted run whose routine has since been deleted must not appear.
    append_persisted_run(
        "deleted-routine-id",
        &PersistedRun {
            workbench: "some-slug-2000".into(),
            started_at: 2000,
            finished_at: 2005,
            status: RunStatus::Success,
            exit_code: Some(0),
        },
    );

    let runs = svc_list_all_runs(&store, None);
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].routine_id, "id");
    assert_eq!(runs[0].status, RunStatus::Failed);
    assert_eq!(runs[0].exit_code, Some(1));
}
