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

/// Build a create request with the given title and an otherwise-valid body.
pub(super) fn create_req_with_title(title: &str) -> CreateRoutineRequest {
    CreateRoutineRequest {
        disabled_reason: None,
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
        power_saving_exempt: false,
        tags: vec![],
        env: std::collections::HashMap::new(),
        failure_threshold: None,
        notifications: Default::default(),
        timezone: None,
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
            disabled_reason: None,
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
            power_saving_exempt: false,
            tags: vec![],
            env: std::collections::HashMap::new(),
            failure_threshold: None,
            notifications: Default::default(),
            timezone: None,
        },
    );
    assert!(matches!(result, Err(AppError::BadRequest(_))));
    // Nothing should have been persisted.
    assert!(store.lock().unwrap().is_empty());
}

/// Build a create request with the given title and `timezone`, otherwise valid.
fn create_req_with_timezone(title: &str, timezone: Option<&str>) -> CreateRoutineRequest {
    CreateRoutineRequest {
        timezone: timezone.map(str::to_string),
        ..create_req_with_title(title)
    }
}

#[test]
fn svc_create_rejects_an_unknown_timezone_name() {
    let _home = TempHome::set();
    // Covers `validate_timezone`'s IANA-lookup reject branch: a string that merely looks like a
    // zone name, but is not present in the on-disk zoneinfo database, 400s rather than being
    // persisted and silently never applied (issue #405).
    let store = new_store();
    let result = svc_create(
        &store,
        create_req_with_timezone(
            "Svc Create Unknown Timezone ZZZ",
            Some("Definitely/Not_A_Zone"),
        ),
    );
    assert!(matches!(result, Err(AppError::BadRequest(_))));
    assert!(store.lock().unwrap().is_empty());
}

#[test]
fn svc_create_rejects_a_path_traversal_timezone_value() {
    let _home = TempHome::set();
    // A crafted value must not reach the filesystem check at all (issue #405).
    let store = new_store();
    let result = svc_create(
        &store,
        create_req_with_timezone(
            "Svc Create Traversal Timezone ZZZ",
            Some("../../etc/passwd"),
        ),
    );
    assert!(matches!(result, Err(AppError::BadRequest(_))));
    assert!(store.lock().unwrap().is_empty());
}

#[test]
fn svc_create_treats_a_blank_timezone_as_unset() {
    let _home = TempHome::set();
    // Mirrors `model`: a blank/whitespace-only value is not an error, it just means "no override".
    let store = new_store();
    let result = svc_create(
        &store,
        create_req_with_timezone("Svc Create Blank Timezone ZZZ", Some("   ")),
    )
    .expect("blank timezone should not be rejected");
    assert_eq!(result.routine.timezone, None);
}

#[cfg(target_os = "linux")]
#[test]
fn svc_create_accepts_a_valid_iana_timezone_on_linux() {
    let _home = TempHome::set();
    // The accept path only runs on Linux — `CRON_TZ` is a vixie-cron/cronie (Linux) extension,
    // so the same request must 400 on any other host (see
    // `svc_create_rejects_timezone_override_on_non_linux_hosts` below).
    let store = new_store();
    let result = svc_create(
        &store,
        create_req_with_timezone("Svc Create Valid Timezone ZZZ", Some("Asia/Jerusalem")),
    )
    .expect("a real IANA zone name should be accepted on Linux");
    assert_eq!(result.routine.timezone.as_deref(), Some("Asia/Jerusalem"));
}

#[cfg(not(target_os = "linux"))]
#[test]
fn svc_create_rejects_timezone_override_on_non_linux_hosts() {
    let _home = TempHome::set();
    // `CRON_TZ` is not honored by BSD `cron` (macOS); even a real IANA zone name must be rejected
    // outright here rather than accepted and then silently ignored at crontab-sync time.
    let store = new_store();
    let result = svc_create(
        &store,
        create_req_with_timezone("Svc Create Timezone Non-Linux ZZZ", Some("Asia/Jerusalem")),
    );
    assert!(matches!(result, Err(AppError::BadRequest(_))));
    assert!(store.lock().unwrap().is_empty());
}
include!("svc_create_accepts_builtin_agent_tests.rs");
