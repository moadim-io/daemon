#![allow(
    clippy::wildcard_imports,
    clippy::too_many_arguments,
    clippy::needless_pass_by_value,
    dead_code,
    reason = "split-out module preserves existing code while keeping files under the linecheck limit"
)]
use super::*;

#[test]
fn record_run_outcome_opts_out_when_threshold_zero() {
    let _home = TempHome::set();
    let store = new_store();
    let mut routine = make_routine("r1", "Breaker Opt Out Zero", Some(0));
    routine.consecutive_failures = 99;
    store.lock().unwrap().insert("r1".into(), routine);

    record_run_outcome(&store, "r1", crate::routines::model::RunStatus::Failed);

    let lock = store.lock().unwrap();
    let routine = lock.get("r1").unwrap();
    assert_eq!(routine.consecutive_failures, 100);
    assert!(routine.enabled, "Some(0) threshold must never auto-disable");
}

#[test]
fn record_run_outcome_logs_a_warning_when_persisting_fails() {
    // Reuse the on-disk slug-collision guard (#188) in `write_routine` as a reliable, filesystem-
    // permission-free way to make the persist step fail: a `routine.toml` already on disk for this
    // slug under a *different* id makes `write_routine` refuse to overwrite it.
    let _home = TempHome::set();
    let store = new_store();
    let title = "Breaker Persist Conflict";
    write_routine(&make_routine("other-id", title, Some(5))).expect("seed conflicting routine");
    store
        .lock()
        .unwrap()
        .insert("r1".into(), make_routine("r1", title, Some(5)));

    // Must not panic even though the persist step fails; the in-memory counter still updates.
    record_run_outcome(&store, "r1", crate::routines::model::RunStatus::Failed);

    assert_eq!(
        store
            .lock()
            .unwrap()
            .get("r1")
            .unwrap()
            .consecutive_failures,
        1
    );
}

#[test]
fn record_run_outcome_resyncs_crontab_successfully_on_auto_disable() {
    let _home = TempHome::set();
    let _cron = OkCronShim::install();
    let store = new_store();
    let mut routine = make_routine("r1", "Breaker Trip Cron Ok", Some(1));
    routine.consecutive_failures = 0;
    store.lock().unwrap().insert("r1".into(), routine);

    record_run_outcome(&store, "r1", crate::routines::model::RunStatus::Failed);

    assert!(!store.lock().unwrap().get("r1").unwrap().enabled);
}

#[test]
fn record_run_outcome_missing_routine_is_a_no_op() {
    let _home = TempHome::set();
    let store = new_store();
    // No routine inserted at all; must return without panicking.
    record_run_outcome(
        &store,
        "does-not-exist",
        crate::routines::model::RunStatus::Failed,
    );
    assert!(store.lock().unwrap().is_empty());
}
