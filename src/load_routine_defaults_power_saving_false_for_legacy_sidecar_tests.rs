
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
