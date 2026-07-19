#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use super::*;

use crate::routines::new_store;

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

#[test]
fn svc_trigger_returns_locked_when_disabled() {
    let _home = TempHome::set();
    let store = new_store();
    let mut routine = make_routine("disabled-trig-id", "Disabled Trigger Test ZZZ", 1, 1);
    routine.enabled = false;
    store
        .lock()
        .unwrap()
        .insert("disabled-trig-id".into(), routine);

    let result = svc_trigger(&store, "disabled-trig-id");
    assert!(
        matches!(result, Err(AppError::Locked(ref msg)) if msg.contains("disabled")),
        "expected a Locked error naming disabled, got {result:?}"
    );
}

#[test]
fn svc_trigger_returns_locked_when_power_saving() {
    let _home = TempHome::set();
    let store = new_store();
    let mut routine = make_routine(
        "power-saving-trig-id",
        "Power Saving Trigger Test ZZZ",
        1,
        1,
    );
    routine.power_saving = true;
    store
        .lock()
        .unwrap()
        .insert("power-saving-trig-id".into(), routine);

    let result = svc_trigger(&store, "power-saving-trig-id");
    assert!(
        matches!(result, Err(AppError::Locked(ref msg)) if msg.contains("power-saving")),
        "expected a Locked error naming power-saving, got {result:?}"
    );
}

#[test]
fn svc_trigger_scheduled_returns_locked_when_disabled() {
    let _home = TempHome::set();
    let store = new_store();
    let mut routine = make_routine("disabled-sched-id", "Disabled Sched Test ZZZ", 1, 1);
    routine.enabled = false;
    store
        .lock()
        .unwrap()
        .insert("disabled-sched-id".into(), routine);

    let result = svc_trigger_scheduled(&store, "disabled-sched-id");
    assert!(
        matches!(result, Err(AppError::Locked(ref msg)) if msg.contains("disabled")),
        "expected a Locked error naming disabled, got {result:?}"
    );
}

#[test]
fn svc_trigger_scheduled_returns_locked_when_power_saving() {
    let _home = TempHome::set();
    let store = new_store();
    let mut routine = make_routine("power-saving-sched-id", "Power Saving Sched Test ZZZ", 1, 1);
    routine.power_saving = true;
    store
        .lock()
        .unwrap()
        .insert("power-saving-sched-id".into(), routine);

    let result = svc_trigger_scheduled(&store, "power-saving-sched-id");
    assert!(
        matches!(result, Err(AppError::Locked(ref msg)) if msg.contains("power-saving")),
        "expected a Locked error naming power-saving, got {result:?}"
    );
}

#[test]
fn svc_set_power_saving_missing_routine_not_found() {
    let _home = TempHome::set();
    assert!(matches!(
        svc_set_power_saving(&new_store(), "nope", true),
        Err(AppError::NotFound)
    ));
}

#[test]
fn svc_set_power_saving_sets_and_clears_without_touching_enabled() {
    let _home = TempHome::set();
    let store = new_store();
    let routine = make_routine("power-saving-set-id", "Power Saving Set Test ZZZ", 1, 1);
    store
        .lock()
        .unwrap()
        .insert("power-saving-set-id".into(), routine);

    let updated = svc_set_power_saving(&store, "power-saving-set-id", true).unwrap();
    assert!(updated.power_saving);
    assert!(updated.enabled, "enabled must be untouched");
    assert!(
        store
            .lock()
            .unwrap()
            .get("power-saving-set-id")
            .unwrap()
            .power_saving
    );

    let updated = svc_set_power_saving(&store, "power-saving-set-id", false).unwrap();
    assert!(!updated.power_saving);
    assert!(updated.enabled);
}

/// Cover the `write_routine(...).map_err(|_| AppError::Internal)?` branch: every other test in
/// this file only exercises the success path, so that `?`'s error arm never ran. A read-only
/// config dir makes `create_private_dir_all` inside `write_routine` fail, mirroring the technique
/// already used for `lock`/`unlock` in `handlers_tests.rs`.
#[test]
fn svc_set_power_saving_returns_internal_on_write_failure() {
    use std::os::unix::fs::PermissionsExt as _;

    let _home = TempHome::set();
    let store = new_store();
    let routine = make_routine(
        "power-saving-write-fail-id",
        "Power Saving Write Fail Test ZZZ",
        1,
        1,
    );
    store
        .lock()
        .unwrap()
        .insert("power-saving-write-fail-id".into(), routine);

    let config_dir = crate::paths::config_dir();
    std::fs::create_dir_all(&config_dir).unwrap();
    let mut perms = std::fs::metadata(&config_dir).unwrap().permissions();
    perms.set_mode(0o555);
    std::fs::set_permissions(&config_dir, perms).unwrap();

    let result = svc_set_power_saving(&store, "power-saving-write-fail-id", true);

    // Restore write permission so `TempHome::drop` can remove the temp tree.
    let mut perms = std::fs::metadata(&config_dir).unwrap().permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(&config_dir, perms).unwrap();

    assert!(
        matches!(result, Err(AppError::Internal)),
        "expected Internal error when write_routine fails, got {result:?}"
    );
}
