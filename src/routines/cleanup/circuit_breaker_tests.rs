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
        consecutive_failures: 0,
        auto_disabled_reason: None,
        tags: vec![],
        ttl_secs: None,
        max_runtime_secs: None,
        failure_threshold,
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

#[test]
fn record_run_outcome_does_not_retrip_or_overwrite_reason_once_disabled() {
    // Once a routine is already auto-disabled, further failures keep incrementing the streak (an
    // operator inspecting it later can see how bad it got) but must not re-run the disable branch
    // or clobber the original reason with a new one every sweep.
    let _home = TempHome::set();
    let store = new_store();
    let mut routine = make_routine("r1", "Breaker Already Tripped", Some(2));
    routine.consecutive_failures = 2;
    routine.enabled = false;
    routine.auto_disabled_reason = Some("auto-disabled after 2 consecutive failed run(s)".into());
    store.lock().unwrap().insert("r1".into(), routine);

    record_run_outcome(&store, "r1", crate::routines::model::RunStatus::Failed);

    let lock = store.lock().unwrap();
    let routine = lock.get("r1").unwrap();
    assert_eq!(routine.consecutive_failures, 3);
    assert!(!routine.enabled);
}

#[test]
fn record_run_outcome_opts_out_when_threshold_none() {
    let _home = TempHome::set();
    let store = new_store();
    let mut routine = make_routine("r1", "Breaker Opt Out None", None);
    routine.consecutive_failures = 99;
    store.lock().unwrap().insert("r1".into(), routine);

    record_run_outcome(&store, "r1", crate::routines::model::RunStatus::Failed);

    let lock = store.lock().unwrap();
    let routine = lock.get("r1").unwrap();
    assert_eq!(routine.consecutive_failures, 100);
    assert!(routine.enabled, "None threshold must never auto-disable");
}

/// A minimal `crontab` shim wired in via `MOADIM_CRONTAB_BIN` that always succeeds, mirroring the
/// pattern in `sync::routines_sync_tests::CronShim` — used here only to exercise the successful
/// crontab-resync path after an auto-disable, since test builds otherwise never touch a real
/// `crontab` (see `crate::sync::crontab_bin`'s doc comment).
struct OkCronShim {
    script: std::path::PathBuf,
    previous: Option<std::ffi::OsString>,
}

impl OkCronShim {
    fn install() -> Self {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("moadim-cbtest-cron-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let script = dir.join("crontab-ok.sh");
        std::fs::write(&script, "#!/bin/sh\nexit 0\n").unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        let previous = std::env::var_os("MOADIM_CRONTAB_BIN");
        // SAFETY: tests in this crate run single-threaded (RUST_TEST_THREADS=1).
        unsafe {
            std::env::set_var("MOADIM_CRONTAB_BIN", &script);
        }
        Self { script, previous }
    }
}

impl Drop for OkCronShim {
    fn drop(&mut self) {
        // SAFETY: single-threaded test execution.
        unsafe {
            match self.previous.take() {
                Some(value) => std::env::set_var("MOADIM_CRONTAB_BIN", value),
                None => std::env::remove_var("MOADIM_CRONTAB_BIN"),
            }
        }
        let _ = std::fs::remove_dir_all(self.script.parent().unwrap());
    }
}

#[allow(
    clippy::missing_docs_in_private_items,
    reason = "split-out module keeps the file under the linecheck limit"
)]
#[path = "circuit_breaker_tests_part2.rs"]
mod circuit_breaker_tests_part2;
