#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use super::*;
use crate::routines::{new_store, slugify};
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
        disabled_reason: None,
        model: None,
        schedule: "@daily".into(),
        schedules: vec![],
        title: "Valid Title".into(),
        agent: "claude".into(),
        prompt: "do the thing".into(),
        goal: None,
        repositories: vec![],
        machines: vec![crate::machine::current_machine()],
        enabled: true,
        ttl_secs: None,
        max_runtime_secs: None,
        power_saving_exempt: false,
        tags: vec![],
        env: std::collections::HashMap::new(),
        failure_threshold: None,
        notifications: Default::default(),
        timezone: None,
    }
}

/// Build a no-op update request (every field `None`); callers set one field.
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
include!("svc_create_rejects_duplicate_slug_tests.rs");
