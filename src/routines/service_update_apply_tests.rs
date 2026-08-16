#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use super::*;
use crate::routines::{new_store, slugify};
use std::sync::Mutex;

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

fn make_routine(id: &str, title: &str, created_at: u64, updated_at: u64) -> Routine {
    Routine {
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
        disabled_reason: None,
        source: "managed".to_string(),
        created_at,
        updated_at,
        last_manual_trigger_at: None,
        last_scheduled_trigger_at: None,
        snoozed_until: None,
        skip_runs: None,
        power_saving: false,
        power_saving_exempt: false,
        tags: vec![],
        ttl_secs: None,
        max_runtime_secs: None,
        env: std::collections::HashMap::new(),
        auto_disabled_reason: None,
        consecutive_failures: 0,
        failure_threshold: None,
        notifications: Default::default(),
        timezone: None,
    }
}

fn empty_update_request() -> UpdateRoutineRequest {
    UpdateRoutineRequest {
        disabled_reason: None,
        model: None,
        schedule: None,
        schedules: None,
        title: None,
        agent: None,
        prompt: None,
        goal: None,
        repositories: None,
        machines: None,
        enabled: None,
        ttl_secs: None,
        max_runtime_secs: None,
        power_saving_exempt: None,
        tags: None,
        env: None,
        failure_threshold: None,
        notifications: Default::default(),
        timezone: None,
    }
}

static PATH_GUARD: Mutex<()> = Mutex::new(());

fn with_empty_path(body: impl FnOnce()) {
    let guard = PATH_GUARD
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let saved = std::env::var_os("PATH");
    std::env::set_var("PATH", "");
    body();
    match saved {
        Some(value) => std::env::set_var("PATH", value),
        None => std::env::remove_var("PATH"),
    }
    drop(guard);
}

#[test]
fn svc_update_sets_ttl_secs() {
    let _home = TempHome::set();
    // Covers the `req.ttl_secs` apply branch in `svc_update`.
    let title = "Svc Update Ttl ZZZ";
    let store = new_store();
    let routine = make_routine("ttl-id", title, 1, 1);
    crate::routine_storage::write_routine(&routine).unwrap();
    store.lock().unwrap().insert("ttl-id".into(), routine);

    // `with_empty_path` keeps the post-update crontab sync from touching the real
    // crontab (issue #175): the update succeeds, the sync just warns.
    // 1800 < the @daily routine's ttl ceiling (min(MAX_TTL_SECS=3600, interval)), so it is a value
    // that is actually in force rather than one silently clamped down (#468).
    with_empty_path(|| {
        let updated = svc_update(
            &store,
            "ttl-id",
            UpdateRoutineRequest {
        disabled_reason: None,
                model: None,
                schedule: None,
                schedules: None,
                title: None,
                agent: None,
                prompt: None,
                goal: None,
                repositories: None,
                machines: None,
                enabled: None,
                ttl_secs: Some(1800),
                max_runtime_secs: None,
                power_saving_exempt: None,
                tags: None,
                env: None,
                failure_threshold: None,
        notifications: Default::default(),
                timezone: None,
            },
        )
        .unwrap();
        assert_eq!(updated.routine.ttl_secs, Some(1800));
    });
}

#[cfg(target_os = "linux")]
#[test]
fn svc_update_sets_timezone_override() {
    let _home = TempHome::set();
    let title = "Svc Update Timezone Set ZZZ";
    let store = new_store();
    let routine = make_routine("tz-set-id", title, 1, 1);
    crate::routine_storage::write_routine(&routine).unwrap();
    store.lock().unwrap().insert("tz-set-id".into(), routine);

    with_empty_path(|| {
        let updated = svc_update(
            &store,
            "tz-set-id",
            UpdateRoutineRequest {
                timezone: Some("Asia/Jerusalem".to_string()),
                ..empty_update_request()
            },
        )
        .unwrap();
        assert_eq!(updated.routine.timezone.as_deref(), Some("Asia/Jerusalem"));
    });
}

#[test]
fn svc_update_clears_an_existing_timezone_override_via_blank_string() {
    let _home = TempHome::set();
    // Mirrors `model`'s clear-on-blank convention. Sets the override directly on the persisted
    // routine (bypassing create-time platform validation) so this exercises the *clear* path on
    // every host, not just Linux.
    let title = "Svc Update Timezone Clear ZZZ";
    let store = new_store();
    let mut routine = make_routine("tz-clear-id", title, 1, 1);
    routine.timezone = Some("Asia/Jerusalem".to_string());
    crate::routine_storage::write_routine(&routine).unwrap();
    store.lock().unwrap().insert("tz-clear-id".into(), routine);

    with_empty_path(|| {
        let updated = svc_update(
            &store,
            "tz-clear-id",
            UpdateRoutineRequest {
                timezone: Some("   ".to_string()),
                ..empty_update_request()
            },
        )
        .unwrap();
        assert_eq!(updated.routine.timezone, None);
    });
}

#[test]
fn svc_update_leaves_timezone_unchanged_when_the_field_is_omitted() {
    let _home = TempHome::set();
    let title = "Svc Update Timezone Untouched ZZZ";
    let store = new_store();
    let mut routine = make_routine("tz-untouched-id", title, 1, 1);
    routine.timezone = Some("Asia/Jerusalem".to_string());
    crate::routine_storage::write_routine(&routine).unwrap();
    store.lock().unwrap().insert("tz-untouched-id".into(), routine);

    with_empty_path(|| {
        let updated = svc_update(
            &store,
            "tz-untouched-id",
            UpdateRoutineRequest {
                ttl_secs: Some(1800),
                ..empty_update_request()
            },
        )
        .unwrap();
        assert_eq!(updated.routine.timezone.as_deref(), Some("Asia/Jerusalem"));
    });
}

#[cfg(test)]
#[path = "service_update_disabled_reason_tests.rs"]
mod service_update_disabled_reason_tests;

include!("svc_update_sets_max_runtime_secs_tests.rs");
