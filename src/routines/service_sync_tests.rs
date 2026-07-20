#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use super::*;

use crate::routines::new_store;
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
fn svc_create_warns_when_crontab_sync_fails() {
    let _home = TempHome::set();
    // With `PATH` cleared the `crontab` binary cannot be spawned, so
    // `sync_routines_to_crontab` errors and `svc_create` logs the warning but
    // still returns the created routine.
    let title = "Svc Create Sync Fail ZZZ";
    let store = new_store();
    with_empty_path(|| {
        let created = svc_create(
            &store,
            CreateRoutineRequest {
                model: None,
                schedule: "@daily".into(),
                title: title.into(),
                agent: "claude".into(),
                prompt: "p".into(),
                goal: None,
                repositories: vec![],
                machines: vec![crate::machine::current_machine()],
                enabled: true,
                ttl_secs: None,
                max_runtime_secs: None,
                tags: vec![],
                env: std::collections::HashMap::new(),
            },
        )
        .unwrap();
        assert_eq!(created.routine.title, title);
    });
}

#[test]
fn svc_create_rejects_goal_over_five_lines() {
    let _home = TempHome::set();
    // A goal is meant to be a glanceable "why" (≤5 lines); a 6-line value is rejected at create
    // time with `BadRequest`, mirroring the other content bounds.
    let store = new_store();
    let result = svc_create(
        &store,
        CreateRoutineRequest {
            schedule: "@daily".into(),
            title: "Svc Create Long Goal ZZZ".into(),
            agent: "claude".into(),
            model: None,
            prompt: "p".into(),
            goal: Some("a\nb\nc\nd\ne\nf".into()),
            repositories: vec![],
            machines: vec![],
            enabled: true,
            ttl_secs: None,
            max_runtime_secs: None,
            tags: vec![],
            env: std::collections::HashMap::new(),
        },
    );
    match result {
        Err(AppError::BadRequest(msg)) => assert!(msg.contains("goal"), "got {msg:?}"),
        other => panic!("expected BadRequest, got {other:?}"),
    }
}

#[test]
fn svc_create_trims_and_persists_goal() {
    let _home = TempHome::set();
    // A present goal is trimmed and stored, and it survives a reload from disk.
    let title = "Svc Create Goal ZZZ";
    let store = new_store();
    with_empty_path(|| {
        let created = svc_create(
            &store,
            CreateRoutineRequest {
                schedule: "@daily".into(),
                title: title.into(),
                agent: "claude".into(),
                model: None,
                prompt: "p".into(),
                goal: Some("  keep the backlog small  ".into()),
                repositories: vec![],
                machines: vec![crate::machine::current_machine()],
                enabled: true,
                ttl_secs: None,
                max_runtime_secs: None,
                tags: vec![],
                env: std::collections::HashMap::new(),
            },
        )
        .unwrap();
        assert_eq!(
            created.routine.goal.as_deref(),
            Some("keep the backlog small")
        );
        // Reloading the store from disk yields the same goal (persisted to routine.toml).
        let reloaded = crate::routine_storage::load_store();
        let stored = reloaded
            .lock()
            .unwrap()
            .get(&created.routine.id)
            .cloned()
            .expect("routine persisted");
        assert_eq!(stored.goal.as_deref(), Some("keep the backlog small"));
    });
}

#[test]
fn svc_update_clears_goal_with_empty_string() {
    let _home = TempHome::set();
    // `Some("")` on update clears the goal; `None` would instead keep the existing value.
    let title = "Svc Update Clear Goal ZZZ";
    let store = new_store();
    let mut routine = make_routine("upd-goal-id", title, 1, 1);
    routine.goal = Some("old goal".into());
    crate::routine_storage::write_routine(&routine).unwrap();
    store.lock().unwrap().insert("upd-goal-id".into(), routine);
    with_empty_path(|| {
        let updated = svc_update(
            &store,
            "upd-goal-id",
            UpdateRoutineRequest {
                schedule: None,
                title: None,
                agent: None,
                model: None,
                prompt: None,
                goal: Some(String::new()),
                repositories: None,
                machines: None,
                enabled: None,
                ttl_secs: None,
                max_runtime_secs: None,
                tags: None,
                env: None,
            },
        )
        .unwrap();
        assert_eq!(updated.routine.goal, None);
    });
}

#[test]
fn svc_update_warns_when_crontab_sync_fails() {
    let _home = TempHome::set();
    // Same crontab-spawn failure as above, on the update path.
    let title = "Svc Update Sync Fail ZZZ";
    let store = new_store();
    let routine = make_routine("upd-sync-id", title, 1, 1);
    crate::routine_storage::write_routine(&routine).unwrap();
    store.lock().unwrap().insert("upd-sync-id".into(), routine);
    with_empty_path(|| {
        let updated = svc_update(
            &store,
            "upd-sync-id",
            UpdateRoutineRequest {
                model: None,
                schedule: None,
                title: None,
                agent: None,
                prompt: Some("changed".into()),
                goal: None,
                repositories: None,
                machines: None,
                enabled: None,
                ttl_secs: None,
                max_runtime_secs: None,
                tags: None,
                env: None,
            },
        )
        .unwrap();
        assert_eq!(updated.routine.prompt, "changed");
    });
}

#[test]
fn svc_delete_warns_when_crontab_sync_fails() {
    let _home = TempHome::set();
    // Same crontab-spawn failure, on the delete path.
    let title = "Svc Delete Sync Fail ZZZ";
    let store = new_store();
    let routine = make_routine("del-sync-id", title, 1, 1);
    crate::routine_storage::write_routine(&routine).unwrap();
    store.lock().unwrap().insert("del-sync-id".into(), routine);
    with_empty_path(|| {
        let deleted = svc_delete(&store, "del-sync-id").unwrap();
        assert_eq!(deleted.routine.title, title);
    });
}
