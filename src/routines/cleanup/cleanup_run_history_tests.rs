//! Tests for `cleanup_expired_workbenches` persisting durable `runs.log` history for a reaped
//! workbench, including the retry guard that stops a run from being recorded twice. Split out of
//! `cleanup_tests.rs` to keep that file under the repo's line-count gate.

#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use super::*;

fn make_routine(id: &str, title: &str) -> super::super::model::Routine {
    super::super::model::Routine {
        model: None,
        id: id.to_string(),
        schedule: "@daily".to_string(),
        title: title.to_string(),
        agent: "claude".to_string(),
        prompt: "do the thing".to_string(),
        goal: None,
        repositories: vec![],
        machines: vec![crate::machine::current_machine()],
        enabled: true,
        source: "managed".to_string(),
        created_at: 1,
        updated_at: 1,
        last_manual_trigger_at: None,
        last_scheduled_trigger_at: None,
        snoozed_until: None,
        skip_runs: None,
        power_saving: false,
        tags: vec![],
        ttl_secs: None,
        max_runtime_secs: None,
        env: std::collections::HashMap::new(),
    }
}

#[test]
fn cleanup_expired_workbenches_persists_run_history_before_removal() {
    // A reaped workbench whose slug matches a *current* routine gets a durable `runs.log` record
    // (via the real `persist` closure in `cleanup_expired_workbenches`), so `svc_list_runs` still
    // knows about it after the workbench itself is gone.
    let home =
        std::env::temp_dir().join(format!("moadim-cleanup-persist-{}", uuid::Uuid::new_v4()));
    let previous = std::env::var_os("MOADIM_HOME_OVERRIDE");
    // SAFETY: tests in this crate run single-threaded (RUST_TEST_THREADS=1); restored below.
    unsafe {
        std::env::set_var("MOADIM_HOME_OVERRIDE", &home);
    }

    let title = "Cleanup Persist ZZQ";
    let slug = super::super::command::slugify(title);
    let store = super::super::model::new_store();
    store
        .lock()
        .unwrap()
        .insert("persist-id".into(), make_routine("persist-id", title));

    let workbenches = crate::paths::workbenches_dir();
    let failed = workbenches.join(format!("{slug}-1"));
    let succeeded = workbenches.join(format!("{slug}-2"));
    let unknown = workbenches.join(format!("{slug}-3"));
    std::fs::create_dir_all(&failed).unwrap();
    std::fs::write(failed.join("exit_code"), "1").unwrap();
    std::fs::create_dir_all(&succeeded).unwrap();
    std::fs::write(succeeded.join("exit_code"), "0").unwrap();
    std::fs::create_dir_all(&unknown).unwrap();
    // No `exit_code` file at all: e.g. the launch aborted before the agent ran.

    let stats = cleanup_expired_workbenches(&store);

    assert_eq!(stats.removed, 3);
    assert!(!failed.exists() && !succeeded.exists() && !unknown.exists());
    let mut history = super::super::run_history::read_persisted_runs("persist-id");
    history.sort_by_key(|run| run.workbench.clone());
    assert_eq!(history.len(), 3);
    assert_eq!(history[0].exit_code, Some(1));
    assert_eq!(history[0].status, super::super::model::RunStatus::Failed);
    assert_eq!(history[1].exit_code, Some(0));
    assert_eq!(history[1].status, super::super::model::RunStatus::Success);
    assert_eq!(history[2].exit_code, None);
    assert_eq!(history[2].status, super::super::model::RunStatus::Unknown);

    // SAFETY: single-threaded harness; restore the saved override.
    unsafe {
        match previous {
            Some(value) => std::env::set_var("MOADIM_HOME_OVERRIDE", value),
            None => std::env::remove_var("MOADIM_HOME_OVERRIDE"),
        }
    }
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn cleanup_expired_workbenches_does_not_duplicate_history_on_retry() {
    // Simulates a workbench whose `remove_dir_all` failed on a prior sweep: it still exists (as if
    // recreated identically) and gets expired again on the next sweep. Without the
    // `has_persisted_run` guard in the `persist` closure, this would append a second `runs.log`
    // record for the same run every sweep the removal keeps failing.
    let home = std::env::temp_dir().join(format!("moadim-cleanup-retry-{}", uuid::Uuid::new_v4()));
    let previous = std::env::var_os("MOADIM_HOME_OVERRIDE");
    // SAFETY: tests in this crate run single-threaded (RUST_TEST_THREADS=1); restored below.
    unsafe {
        std::env::set_var("MOADIM_HOME_OVERRIDE", &home);
    }

    let title = "Cleanup Retry ZZQ";
    let slug = super::super::command::slugify(title);
    let store = super::super::model::new_store();
    store
        .lock()
        .unwrap()
        .insert("retry-id".into(), make_routine("retry-id", title));

    let workbenches = crate::paths::workbenches_dir();
    let workbench = workbenches.join(format!("{slug}-1"));
    std::fs::create_dir_all(&workbench).unwrap();
    std::fs::write(workbench.join("exit_code"), "0").unwrap();

    let first = cleanup_expired_workbenches(&store);
    assert_eq!(first.removed, 1);
    assert!(!workbench.exists());
    assert_eq!(
        super::super::run_history::read_persisted_runs("retry-id").len(),
        1
    );

    // Recreate the identical workbench, standing in for a removal that failed and left it in
    // place for the next sweep.
    std::fs::create_dir_all(&workbench).unwrap();
    std::fs::write(workbench.join("exit_code"), "0").unwrap();

    let second = cleanup_expired_workbenches(&store);
    assert_eq!(second.removed, 1);
    let history = super::super::run_history::read_persisted_runs("retry-id");
    assert_eq!(
        history.len(),
        1,
        "the retry must not append a duplicate history entry for the same workbench"
    );

    // SAFETY: single-threaded harness; restore the saved override.
    unsafe {
        match previous {
            Some(value) => std::env::set_var("MOADIM_HOME_OVERRIDE", value),
            None => std::env::remove_var("MOADIM_HOME_OVERRIDE"),
        }
    }
    let _ = std::fs::remove_dir_all(&home);
}
