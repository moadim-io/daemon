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
        tags: vec![],
        ttl_secs: None,
        max_runtime_secs: None,
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
            !toml_text.contains("power_saving"),
            "routine.toml must not carry power-saving state: {toml_text}"
        );
        assert!(crate::paths::routine_state_path(&slug).exists());
        let state_text = std::fs::read_to_string(crate::paths::routine_state_path(&slug)).unwrap();
        assert!(state_text.contains("power_saving"));
        assert!(load_routine_from_dir(&slug).unwrap().power_saving);
    });
}

#[test]
fn load_routine_defaults_power_saving_false_for_legacy_sidecar() {
    // A `state.local.toml` written before `power_saving` existed (e.g. only carrying a manual
    // trigger timestamp) must still load, defaulting the new field to `false` rather than failing
    // to parse — the same upgrade-safety guarantee the other sidecar fields already have.
    with_override_home(|_home| {
        let title = "Rs Legacy Sidecar Routine";
        let slug = slugify(title);
        write_routine(&make_routine("rs-legacy-sidecar-id", title)).unwrap();
        std::fs::write(
            crate::paths::routine_state_path(&slug),
            "last_manual_trigger_at = 111\n",
        )
        .unwrap();

        let loaded = load_routine_from_dir(&slug).unwrap();
        assert_eq!(loaded.last_manual_trigger_at, Some(111));
        assert!(!loaded.power_saving);
    });
}

#[test]
fn write_routine_errors_when_state_sidecar_path_is_occupied_by_a_directory() {
    // `write_runtime_state`'s `atomic_write(&path, ...)` (routine_storage.rs) is the last
    // fallible step in `write_routine`, but nothing exercised its own error branch — only
    // `atomic_write`'s internal rename failure is covered directly (see
    // `utils::atomic_tests::errors_and_cleans_up_when_rename_fails`), never `write_routine`
    // observing and propagating that failure through its own `?`. Reuse the same
    // directory-occupies-target-path technique: a pre-existing directory at
    // `state.local.toml`'s path makes the rename inside `atomic_write` fail, so
    // `write_runtime_state` (reached because `power_saving = true` skips the no-op early
    // return) surfaces an `Err` instead of silently succeeding.
    with_override_home(|_home| {
        let title = "Rs Sidecar Occupied Routine";
        let slug = slugify(title);
        let mut routine = make_routine("rs-sidecar-occupied-id", title);
        routine.power_saving = true;

        let state_path = crate::paths::routine_state_path(&slug);
        std::fs::create_dir_all(&state_path).unwrap();

        let err = write_routine(&routine).unwrap_err();
        assert!(
            state_path.is_dir(),
            "the occupying directory must be left untouched: {err}"
        );
    });
}

#[test]
fn write_routine_clears_stale_sidecar_when_power_saving_cleared() {
    with_override_home(|_home| {
        let title = "Rs Clear Power Saving Routine";
        let slug = slugify(title);
        let mut routine = make_routine("rs-clear-power-saving-id", title);
        routine.power_saving = true;
        write_routine(&routine).unwrap();
        assert!(crate::paths::routine_state_path(&slug).exists());

        routine.power_saving = false;
        write_routine(&routine).unwrap();
        assert!(
            !crate::paths::routine_state_path(&slug).exists(),
            "sidecar should be removed once power saving clears and no other runtime state remains"
        );
        assert!(!load_routine_from_dir(&slug).unwrap().power_saving);
    });
}
