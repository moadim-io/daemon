#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use super::*;

use crate::routines::new_store;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Point `MOADIM_HOME_OVERRIDE` at a fresh, empty temp home for the duration of a test, removing the
/// env var and the temp dir on drop. This keeps `svc_create`/`svc_update`/`write_routine` and the
/// other disk-touching paths off the developer's real `~/.moadim`, so a panicking assertion can never
/// leak test routines into the real home. Tests in this crate run single-threaded
/// (`RUST_TEST_THREADS=1`), so the global env mutation is safe.
struct TempHome(std::path::PathBuf);

impl TempHome {
    fn set() -> Self {
        let dir = std::env::temp_dir().join(format!("moadim-svctest-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("create temp home");
        // SAFETY: single-threaded test execution.
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

/// Build a routine with overridable identity, title, timestamps, and repository URL.
fn make_routine(id: &str, title: &str, created_at: u64, updated_at: u64) -> Routine {
    Routine {
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
        created_at,
        updated_at,
        last_manual_trigger_at: None,
        last_scheduled_trigger_at: None,
        snoozed_until: None,
        skip_runs: None,
        power_saving: false,
        tags: vec![],
        ttl_secs: None,
        max_runtime_secs: None,
    }
}

/// Wrap a list of routines into a populated [`RoutineStore`].
fn store_with(routines: Vec<Routine>) -> RoutineStore {
    let mut map = HashMap::new();
    for routine in routines {
        map.insert(routine.id.clone(), routine);
    }
    Arc::new(Mutex::new(map))
}

/// Build a minimal valid create request; callers tweak the field under test.
fn valid_create_request() -> CreateRoutineRequest {
    CreateRoutineRequest {
        model: None,
        goal: None,
        schedule: "@daily".into(),
        title: "Valid Title".into(),
        agent: "claude".into(),
        prompt: "do the thing".into(),
        repositories: vec![],
        machines: vec![crate::machine::current_machine()],
        enabled: true,
        ttl_secs: None,
        max_runtime_secs: None,
        tags: vec![],
    }
}

/// Build a no-op update request (every field `None`); callers set one field.
fn empty_update_request() -> UpdateRoutineRequest {
    UpdateRoutineRequest {
        model: None,
        goal: None,
        schedule: None,
        title: None,
        agent: None,
        prompt: None,
        repositories: None,
        machines: None,
        enabled: None,
        ttl_secs: None,
        max_runtime_secs: None,
        tags: None,
    }
}

#[test]
fn svc_create_rejects_ttl_above_cron_ceiling() {
    let _home = TempHome::set();
    // A `*/5 * * * *` routine has a ttl ceiling of min(3600, 300) = 300s. An explicit 1800 would be
    // silently clamped to 300, so it is rejected with `BadRequest` up front (#468).
    let store = new_store();
    let result = svc_create(
        &store,
        CreateRoutineRequest {
            model: None,
            goal: None,
            schedule: "*/5 * * * *".into(),
            ttl_secs: Some(1800),
            ..valid_create_request()
        },
    );
    assert!(matches!(result, Err(AppError::BadRequest(_))));
}

#[test]
fn svc_create_rejects_max_runtime_above_cron_ceiling() {
    let _home = TempHome::set();
    // Mirror of the ttl ceiling for the watchdog bound (#468).
    let store = new_store();
    let result = svc_create(
        &store,
        CreateRoutineRequest {
            model: None,
            goal: None,
            schedule: "*/5 * * * *".into(),
            max_runtime_secs: Some(1800),
            ..valid_create_request()
        },
    );
    assert!(matches!(result, Err(AppError::BadRequest(_))));
}

#[test]
fn svc_create_accepts_secs_at_cron_ceiling() {
    let _home = TempHome::set();
    // A value equal to the cron-derived ceiling (`*/5` -> 300s) is in force, not clamped, so it
    // passes `reject_over_ceiling` (covering the `secs <= ceiling` arm for both fields). A
    // duplicate-slug routine pre-seeded in the store makes the create fail *after* that check with a
    // `Conflict`, so the assertion proves the ceiling check did not reject — without performing any
    // crontab/disk mutation.
    let store = store_with(vec![make_routine(
        "at-ceiling-dupe",
        "At Ceiling ZZZ",
        1,
        1,
    )]);
    let result = svc_create(
        &store,
        CreateRoutineRequest {
            model: None,
            goal: None,
            schedule: "*/5 * * * *".into(),
            // Same slug as the pre-seeded routine.
            title: "  at   ceiling ZZZ ".into(),
            ttl_secs: Some(300),
            max_runtime_secs: Some(300),
            ..valid_create_request()
        },
    );
    assert!(matches!(result, Err(AppError::Conflict(_))));
}

#[test]
fn svc_update_rejects_ttl_above_current_schedule_ceiling() {
    let _home = TempHome::set();
    // No schedule supplied: the ceiling derives from the routine's *current* `*/5` schedule, so a
    // 1800s ttl exceeds the 300s ceiling and is rejected without mutating the store (#468).
    let store = store_with(vec![Routine {
        schedule: "*/5 * * * *".to_string(),
        ..make_routine("upd-ttl-ceiling", "Keep Ceiling", 1, 1)
    }]);
    let result = svc_update(
        &store,
        "upd-ttl-ceiling",
        UpdateRoutineRequest {
            model: None,
            goal: None,
            ttl_secs: Some(1800),
            ..empty_update_request()
        },
    );
    assert!(matches!(result, Err(AppError::BadRequest(_))));
    // The store value is untouched by the rejected update.
    assert_eq!(
        store
            .lock()
            .unwrap()
            .get("upd-ttl-ceiling")
            .unwrap()
            .ttl_secs,
        None
    );
}

#[test]
fn svc_update_rejects_secs_above_new_schedule_ceiling() {
    let _home = TempHome::set();
    // A supplied schedule is the *effective* schedule for the ceiling: tightening a `@daily` routine
    // to `*/5` while setting max_runtime 1800 exceeds the new 300s ceiling and is rejected (#468).
    let store = store_with(vec![make_routine("upd-new-sched", "Keep New Sched", 1, 1)]);
    let result = svc_update(
        &store,
        "upd-new-sched",
        UpdateRoutineRequest {
            model: None,
            goal: None,
            schedule: Some("*/5 * * * *".into()),
            max_runtime_secs: Some(1800),
            ..empty_update_request()
        },
    );
    assert!(matches!(result, Err(AppError::BadRequest(_))));
}
