#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use super::*;

use crate::routines::new_store;

pub(super) struct TempHome(std::path::PathBuf);

impl TempHome {
    pub(super) fn set() -> Self {
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

pub(super) fn make_routine(id: &str, title: &str, created_at: u64, updated_at: u64) -> Routine {
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

/// Build a create request with the given title and an otherwise-valid body.
pub(super) fn create_req_with_title(title: &str) -> CreateRoutineRequest {
    CreateRoutineRequest {
        model: None,
        schedule: "@daily".into(),
        schedules: vec![],
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
        failure_threshold: None,
    }
}

#[test]
fn svc_create_rejects_blank_and_punctuation_titles() {
    let _home = TempHome::set();
    // Covers `validate_title`'s alphanumeric-required reject branch via `svc_create`:
    // empty, whitespace-only, and punctuation-only titles all 400 before any
    // persistence or crontab sync, leaving the store empty (issue #226).
    for title in ["", "   \n\t", "!!!"] {
        let store = new_store();
        let result = svc_create(&store, create_req_with_title(title));
        assert!(
            matches!(result, Err(AppError::BadRequest(_))),
            "title {title:?} should be rejected"
        );
        assert!(store.lock().unwrap().is_empty());
    }
}

#[test]
fn svc_create_rejects_overlong_title() {
    let _home = TempHome::set();
    // Covers `validate_title`'s max-length reject branch: a title past
    // `MAX_TITLE_LEN` characters 400s even though it has alphanumerics.
    let store = new_store();
    let title = "a".repeat(MAX_TITLE_LEN + 1);
    let result = svc_create(&store, create_req_with_title(&title));
    assert!(matches!(result, Err(AppError::BadRequest(_))));
    assert!(store.lock().unwrap().is_empty());
}

#[test]
fn svc_create_rejects_unknown_agent() {
    let _home = TempHome::set();
    // Covers the agent-validation branch in `svc_create`: an agent name that is
    // not in the registry must fail loud with `BadRequest` instead of being
    // persisted and silently skipped at fire time (#139).
    let store = new_store();
    let result = svc_create(
        &store,
        CreateRoutineRequest {
            model: None,
            schedule: "@daily".into(),
            schedules: vec![],
            title: "Svc Create Unknown Agent ZZZ".into(),
            agent: "no-such-agent-zzz".into(),
            prompt: "p".into(),
            goal: None,
            repositories: vec![],
            machines: vec![crate::machine::current_machine()],
            enabled: true,
            ttl_secs: None,
            max_runtime_secs: None,
            tags: vec![],
            env: std::collections::HashMap::new(),
            failure_threshold: None,
        },
    );
    assert!(matches!(result, Err(AppError::BadRequest(_))));
    // Nothing should have been persisted.
    assert!(store.lock().unwrap().is_empty());
}
include!("svc_create_accepts_builtin_agent_tests.rs");
