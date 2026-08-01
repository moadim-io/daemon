#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use super::*;
use crate::routines::{slugify, Routine};

fn scratch_dir(tag: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("moadim-rs-{tag}-{}", uuid::Uuid::new_v4()))
}

fn with_override_home(body: impl FnOnce(&std::path::Path)) {
    let home = scratch_dir("override-home");
    std::fs::create_dir_all(&home).unwrap();
    let previous = std::env::var_os("MOADIM_HOME_OVERRIDE");
    // SAFETY: tests in this crate run single-threaded per binary.
    unsafe {
        std::env::set_var("MOADIM_HOME_OVERRIDE", &home);
    }
    body(&home);
    // SAFETY: single-threaded test execution.
    unsafe {
        match previous {
            Some(value) => std::env::set_var("MOADIM_HOME_OVERRIDE", value),
            None => std::env::remove_var("MOADIM_HOME_OVERRIDE"),
        }
    }
    let _ = std::fs::remove_dir_all(&home);
}

fn make_routine(id: &str, title: &str) -> Routine {
    Routine {
        model: None,
        id: id.to_string(),
        schedule: "@daily".to_string(),
        schedules: vec![],
        title: title.to_string(),
        agent: "claude".to_string(),
        prompt: "task".to_string(),
        goal: None,
        repositories: vec![],
        machines: vec![crate::machine::current_machine()],
        enabled: true,
        source: "managed".to_string(),
        created_at: 5,
        updated_at: 6,
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
    }
}

#[test]
fn last_manual_trigger_at_persists_to_log_not_routine_toml() {
    // Manual trigger history is written to the gitignored `manual.log` append-only file and kept
    // out of the version-controlled `routine.toml`; it round-trips through load.
    with_override_home(|_home| {
        let title = "Rs Sidecar Routine";
        let slug = slugify(title);
        let mut routine = make_routine("rs-sidecar-id", title);
        routine.last_manual_trigger_at = Some(12345);
        write_routine(&routine).unwrap();
        // Simulate what svc_trigger does: append to manual.log.
        crate::routine_storage::append_manual_trigger_log(&slug, 12345);

        // The tracked config file does not carry the runtime timestamp...
        let toml_text = std::fs::read_to_string(crate::paths::routine_toml_path(&slug)).unwrap();
        assert!(
            !toml_text.contains("last_manual_trigger_at"),
            "routine.toml must not carry runtime trigger state: {toml_text}"
        );
        // ...the gitignored log does, and it round-trips through load.
        assert!(crate::paths::routine_manual_log_path(&slug).exists());
        let log_text =
            std::fs::read_to_string(crate::paths::routine_manual_log_path(&slug)).unwrap();
        assert!(
            log_text.trim() == "12345",
            "manual.log must contain the timestamp: {log_text}"
        );
        assert_eq!(
            load_routine_from_dir(&slug).unwrap().last_manual_trigger_at,
            Some(12345)
        );
    });
}

#[test]
fn write_routine_clears_stale_sidecar_when_untriggered() {
    // Re-writing a routine with no snooze/skip-runs state removes the state sidecar; an absent
    // manual.log means last_manual_trigger_at round-trips as None.
    with_override_home(|_home| {
        let title = "Rs Clear Sidecar Routine";
        let slug = slugify(title);
        let mut routine = make_routine("rs-clear-id", title);
        // Write with no snooze/skip_runs — sidecar should not be created.
        write_routine(&routine).unwrap();
        assert!(
            !crate::paths::routine_state_path(&slug).exists(),
            "state.local.toml must not be written when there is no snooze/skip-runs state"
        );

        // Snooze it so the sidecar is created, then clear the snooze.
        routine.snoozed_until = Some(9999);
        write_routine(&routine).unwrap();
        assert!(crate::paths::routine_state_path(&slug).exists());

        routine.snoozed_until = None;
        write_routine(&routine).unwrap();
        assert!(
            !crate::paths::routine_state_path(&slug).exists(),
            "sidecar should be removed when there is no snooze/skip-runs state"
        );
        // No manual.log was ever written, so last_manual_trigger_at is None.
        assert_eq!(
            load_routine_from_dir(&slug).unwrap().last_manual_trigger_at,
            None
        );
    });
}

#[test]
fn power_saving_persists_to_sidecar_not_routine_toml() {
    // Power saving is daemon/policy-owned runtime state, like `last_manual_trigger_at`: it lives in
    // the gitignored `state.local.toml` sidecar, not the version-controlled `routine.toml`.
    with_override_home(|_home| {
        let title = "Rs Power Saving Routine";
        let slug = slugify(title);
        let mut routine = make_routine("rs-power-saving-id", title);
        routine.power_saving = true;
        write_routine(&routine).unwrap();

        let toml_text = std::fs::read_to_string(crate::paths::routine_toml_path(&slug)).unwrap();
        assert!(
            !toml_text.contains("power_saving ="),
            "routine.toml must not carry daemon power-saving state: {toml_text}"
        );
        assert!(crate::paths::routine_state_path(&slug).exists());
        let state_text = std::fs::read_to_string(crate::paths::routine_state_path(&slug)).unwrap();
        assert!(state_text.contains("power_saving"));
        assert!(load_routine_from_dir(&slug).unwrap().power_saving);
    });
}
include!("load_routine_defaults_power_saving_false_for_legacy_sidecar_tests.rs");
