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

fn empty_update_request() -> UpdateRoutineRequest {
    UpdateRoutineRequest {
        model: None,
        schedule: None,
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
                model: None,
                schedule: None,
                title: None,
                agent: None,
                prompt: None,
                goal: None,
                repositories: None,
                machines: None,
                enabled: None,
                ttl_secs: Some(1800),
                max_runtime_secs: None,
                tags: None,
                env: None,
            },
        )
        .unwrap();
        assert_eq!(updated.routine.ttl_secs, Some(1800));
    });
}

#[test]
fn svc_update_sets_max_runtime_secs() {
    let _home = TempHome::set();
    // Covers the `req.max_runtime_secs` apply branch in `svc_update`.
    let title = "Svc Update Max Runtime ZZZ";
    let store = new_store();
    let routine = make_routine("max-runtime-id", title, 1, 1);
    crate::routine_storage::write_routine(&routine).unwrap();
    store
        .lock()
        .unwrap()
        .insert("max-runtime-id".into(), routine);

    // `with_empty_path` keeps the post-update crontab sync from touching the real
    // crontab (issue #175): the update succeeds, the sync just warns.
    with_empty_path(|| {
        let updated = svc_update(
            &store,
            "max-runtime-id",
            UpdateRoutineRequest {
                model: None,
                schedule: None,
                title: None,
                agent: None,
                prompt: None,
                goal: None,
                repositories: None,
                machines: None,
                enabled: None,
                ttl_secs: None,
                max_runtime_secs: Some(1234),
                tags: None,
                env: None,
            },
        )
        .unwrap();
        assert_eq!(updated.routine.max_runtime_secs, Some(1234));
    });
}

#[test]
fn svc_update_sets_env() {
    let _home = TempHome::set();
    // Covers the `req.env` validate + apply branches in `svc_update` (#408).
    let title = "Svc Update Env ZZZ";
    let store = new_store();
    let routine = make_routine("env-id", title, 1, 1);
    crate::routine_storage::write_routine(&routine).unwrap();
    store.lock().unwrap().insert("env-id".into(), routine);

    with_empty_path(|| {
        let updated = svc_update(
            &store,
            "env-id",
            UpdateRoutineRequest {
                env: Some(std::collections::HashMap::from([(
                    "MODEL_OVERRIDE".to_string(),
                    "gpt-x".to_string(),
                )])),
                ..empty_update_request()
            },
        )
        .unwrap();
        assert_eq!(
            updated
                .routine
                .env
                .get("MODEL_OVERRIDE")
                .map(String::as_str),
            Some("gpt-x")
        );
    });
}

#[test]
fn svc_update_trims_title_before_persisting() {
    // Covers the title `.trim()` on the `svc_update` apply path. Renaming with the
    // same slug but different spacing/case must store the trimmed title.
    let title = "Svc Update Trim ZZZ";
    let store = new_store();
    let routine = make_routine("trim-id", title, 1, 1);
    crate::routine_storage::write_routine(&routine).unwrap();
    store.lock().unwrap().insert("trim-id".into(), routine);

    with_empty_path(|| {
        let updated = svc_update(
            &store,
            "trim-id",
            UpdateRoutineRequest {
                // Same slug, padded: applies the rename branch without a conflict.
                title: Some("  Svc Update Trim ZZZ  ".into()),
                ..empty_update_request()
            },
        )
        .unwrap();
        assert_eq!(updated.routine.title, title);
    });

    let _ = crate::routine_storage::remove_routine_dir(&slugify(title));
}
