#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

//! Split out of `service_tests.rs` to keep that file under the repo's 700-line
//! pre-push line-count gate (`.githooks/pre-push`).

use super::*;

use crate::routines::new_store;
use std::sync::Mutex;

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

/// Build a no-op update request (every field `None`); callers set one field.
fn empty_update_request() -> UpdateRoutineRequest {
    UpdateRoutineRequest {
        disabled_reason: None,
        model: None,
        goal: None,
        schedule: None,
        schedules: None,
        title: None,
        agent: None,
        prompt: None,
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

/// Serializes the tests that clear `PATH`, so concurrent service tests never see
/// a stripped environment. The poisoned-lock case is recovered into the guard.
static PATH_GUARD: Mutex<()> = Mutex::new(());

/// Run `body` with an empty `PATH`, restoring the original value afterwards.
///
/// Clearing `PATH` makes the `crontab` and `sh` lookups inside the crontab sync
/// and the trigger spawn fail to launch, exercising their warning branches.
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
fn svc_update_not_found_when_no_schedule_and_id_missing() {
    let _home = TempHome::set();
    // Covers L359 error arm: `lock.get(id).ok_or(AppError::NotFound)?` fires when the
    // routine does not exist in the store and no new schedule is provided.
    let store = new_store(); // empty store
    with_empty_path(|| {
        let result = svc_update(
            &store,
            "nonexistent-id",
            UpdateRoutineRequest {
        disabled_reason: None,
                model: None,
                goal: None,
                schedule: None,
                schedules: None,
                ..empty_update_request()
            },
        );
        assert!(matches!(result, Err(AppError::NotFound)));
    });
}

#[test]
fn svc_update_not_found_when_routine_toml_was_deleted_from_disk() {
    let _home = TempHome::set();
    // A routine known to the store whose routine.toml vanished (e.g. a git pull moved it) must
    // map the storage NotFound to AppError::NotFound and must not be recreated on disk.
    let store = new_store();
    let routine = Routine {
        model: None,
        id: "update-deleted-id".to_string(),
        schedule: "@daily".to_string(),
        schedules: vec![],
        title: "Update Deleted ZZZ".to_string(),
        agent: "claude".to_string(),
        prompt: "do the thing".to_string(),
        goal: None,
        repositories: vec![],
        machines: vec![],
        enabled: true,
        disabled_reason: None,
        source: "managed".to_string(),
        created_at: 1,
        updated_at: 1,
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
    };
    crate::routine_storage::write_routine(&routine).unwrap();
    let rel_dir = crate::routine_storage::routine_rel_dir(&routine);
    let toml = crate::paths::routine_toml_path(&rel_dir);
    std::fs::remove_file(&toml).unwrap();
    store
        .lock()
        .unwrap()
        .insert("update-deleted-id".into(), routine);
    with_empty_path(|| {
        let result = svc_update(
            &store,
            "update-deleted-id",
            UpdateRoutineRequest {
                timezone: Some("Asia/Jerusalem".to_string()),
                ..empty_update_request()
            },
        );
        assert!(matches!(result, Err(AppError::NotFound)), "{result:?}");
    });
    assert!(!toml.exists(), "routine.toml must not be recreated");
}

#[test]
fn svc_update_not_found_when_schedule_provided_and_id_missing() {
    let _home = TempHome::set();
    // Covers L371 error arm: `lock.get_mut(id).ok_or(AppError::NotFound)?` fires when
    // the routine does not exist in the store and a new schedule is provided.
    let store = new_store(); // empty store
    with_empty_path(|| {
        let result = svc_update(
            &store,
            "nonexistent-id",
            UpdateRoutineRequest {
        disabled_reason: None,
                model: None,
                goal: None,
                schedule: Some("@daily".into()),
                schedules: None,
                ..empty_update_request()
            },
        );
        assert!(matches!(result, Err(AppError::NotFound)));
    });
}
