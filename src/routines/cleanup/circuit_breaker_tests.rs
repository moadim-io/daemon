//! Tests for the failure circuit-breaker's `record_run_outcome` (#521): increment-on-fail,
//! reset-on-success, trip-at-threshold, and the opt-out paths (`None`/`Some(0)` threshold).

#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use super::*;
use crate::routines::model::new_store;

struct TempHome(std::path::PathBuf);

impl TempHome {
    fn set() -> Self {
        let dir = std::env::temp_dir().join(format!("moadim-cbtest-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("create temp home");
        // SAFETY: tests in this crate run single-threaded (RUST_TEST_THREADS=1).
        unsafe {
            std::env::set_var("MOADIM_HOME_OVERRIDE", &dir);
        }
        Self(dir)
    }
}

impl Drop for TempHome {
    fn drop(&mut self) {
        // SAFETY: single-threaded test execution.
        unsafe {
            std::env::remove_var("MOADIM_HOME_OVERRIDE");
        }
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn make_routine(
    id: &str,
    title: &str,
    failure_threshold: Option<u32>,
) -> crate::routines::model::Routine {
    crate::routines::model::Routine {
        model: None,
        id: id.to_string(),
        schedule: "@daily".to_string(),
        schedules: vec![],
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
        power_saving_exempt: false,
        consecutive_failures: 0,
        auto_disabled_reason: None,
        tags: vec![],
        ttl_secs: None,
        max_runtime_secs: None,
        failure_threshold,
        notifications: Default::default(),
        env: std::collections::HashMap::new(),
    }
}

#[test]
fn record_run_outcome_increments_on_failure() {
    let _home = TempHome::set();
    let store = new_store();
    store
        .lock()
        .unwrap()
        .insert("r1".into(), make_routine("r1", "Breaker Fail", Some(5)));

    record_run_outcome(&store, "r1", crate::routines::model::RunStatus::Failed);

    let lock = store.lock().unwrap();
    let routine = lock.get("r1").unwrap();
    assert_eq!(routine.consecutive_failures, 1);
    assert!(routine.enabled);
    assert!(routine.auto_disabled_reason.is_none());
}

#[test]
fn record_run_outcome_counts_unknown_as_failure() {
    // `Unknown` (session gone, no exit code — e.g. force-killed for hanging) counts toward the
    // streak too: a routine that only ever hangs is exactly the loop this breaker exists to stop.
    let _home = TempHome::set();
    let store = new_store();
    store
        .lock()
        .unwrap()
        .insert("r1".into(), make_routine("r1", "Breaker Unknown", Some(5)));

    record_run_outcome(&store, "r1", crate::routines::model::RunStatus::Unknown);

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
fn record_run_outcome_resets_on_success() {
    let _home = TempHome::set();
    let store = new_store();
    let mut routine = make_routine("r1", "Breaker Reset", Some(5));
    routine.consecutive_failures = 3;
    store.lock().unwrap().insert("r1".into(), routine);

    record_run_outcome(&store, "r1", crate::routines::model::RunStatus::Success);

    assert_eq!(
        store
            .lock()
            .unwrap()
            .get("r1")
            .unwrap()
            .consecutive_failures,
        0
    );
}

#[test]
fn record_run_outcome_trips_at_threshold() {
    let _home = TempHome::set();
    let store = new_store();
    let mut routine = make_routine("r1", "Breaker Trip", Some(3));
    routine.consecutive_failures = 2;
    store.lock().unwrap().insert("r1".into(), routine);

    record_run_outcome(&store, "r1", crate::routines::model::RunStatus::Failed);

    let lock = store.lock().unwrap();
    let routine = lock.get("r1").unwrap();
    assert_eq!(routine.consecutive_failures, 3);
    assert!(
        !routine.enabled,
        "must auto-disable on crossing the threshold"
    );
    assert!(
        routine
            .auto_disabled_reason
            .as_deref()
            .is_some_and(|reason| reason.contains('3')),
        "auto_disabled_reason should name the threshold that tripped: {:?}",
        routine.auto_disabled_reason
    );
}
include!("record_run_outcome_does_not_retrip_or_overwrite_reason_once_disabled_tests.rs");
