//! Tests for `cleanup_expired_workbenches` persisting a watchdog-killed run as `Killed` with its
//! kill reason. Split out of `cleanup_run_history_tests.rs` to keep that file under the line-count
//! gate; reuses its `make_routine` fixture.

#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use super::cleanup_run_history_tests::make_routine;
use super::*;

#[test]
fn cleanup_expired_workbenches_persists_watchdog_kill_as_killed() {
    // A workbench the max-runtime watchdog force-killed carries the `killed` sentinel instead of an
    // exit code. The reaper must persist it as `Killed` with its kill reason, not `Unknown`.
    let home = std::env::temp_dir().join(format!("moadim-cleanup-killed-{}", uuid::Uuid::new_v4()));
    let previous = std::env::var_os("MOADIM_HOME_OVERRIDE");
    // SAFETY: tests in this crate run single-threaded (RUST_TEST_THREADS=1); restored below.
    unsafe {
        std::env::set_var("MOADIM_HOME_OVERRIDE", &home);
    }

    let title = "Cleanup Killed ZZQ";
    let slug = super::super::command::slugify(title);
    let store = super::super::model::new_store();
    store
        .lock()
        .unwrap()
        .insert("killed-id".into(), make_routine("killed-id", title));

    let workbench = crate::paths::workbenches_dir().join(format!("{slug}-7"));
    std::fs::create_dir_all(&workbench).unwrap();
    std::fs::write(workbench.join("exit_code"), "killed\n").unwrap();

    let stats = cleanup_expired_workbenches(&store);
    assert_eq!(stats.removed, 1);

    let history = super::super::run_history::read_persisted_runs("killed-id");
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].status, super::super::model::RunStatus::Killed);
    assert_eq!(history[0].exit_code, None);
    assert_eq!(
        history[0].kill_reason,
        Some(super::super::model::KillReason::MaxRuntime)
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
