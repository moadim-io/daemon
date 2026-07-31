#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use super::*;

use crate::routines::{new_store, slugify};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

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
        env: std::collections::HashMap::new(),
        auto_disabled_reason: None,
        consecutive_failures: 0,
        failure_threshold: None,
    }
}

/// Wrap a list of routines into a populated [`RoutineStore`], and persist each one to disk (under
/// the active `TempHome`) so `svc_list`'s reload-from-disk doesn't wipe the fixture the test just
/// built — disk is the source of truth the reload re-scans on every call.
fn store_with(routines: Vec<Routine>) -> RoutineStore {
    let mut map = HashMap::new();
    for routine in routines {
        write_routine(&routine).expect("write_routine");
        map.insert(routine.id.clone(), routine);
    }
    Arc::new(Mutex::new(map))
}

fn valid_create_request() -> CreateRoutineRequest {
    CreateRoutineRequest {
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
        tags: vec![],
        env: std::collections::HashMap::new(),
        failure_threshold: None,
    }
}

fn empty_update_request() -> UpdateRoutineRequest {
    UpdateRoutineRequest {
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
        tags: None,
        env: None,
        failure_threshold: None,
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

// ─── New tests for previously uncovered lines ────────────────────────────────

#[test]
fn svc_list_local_only_filters_non_matching_machines() {
    let _home = TempHome::set();
    // Covers L137: the `local_only=true` retain path. A routine whose `machines` list
    // does not contain the current machine is dropped; one that does is kept.
    let local = make_routine("list-local-id", "List Local Machine ZZZ", 1, 1);
    // `make_routine` seeds `machines: [current_machine()]`, so this one passes the filter.
    let mut other = make_routine("list-other-id", "List Other Machine ZZZ", 2, 2);
    other.machines = vec!["definitely-not-this-machine-xyz".to_string()];
    let store = store_with(vec![local, other]);
    let query = RoutineListQuery {
        local_only: Some(true),
        ..Default::default()
    };
    let list = svc_list(&store, &crate::paths::routines_dir(), &query);
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].routine.id, "list-local-id");
}
include!("service_coverage_tests_part3.rs");
