
#[test]
fn write_routine_fails_on_runtime_state_write_error() {
    // Covers L190 and L206: `write_runtime_state(..)? ` and the `atomic_write` inside it.
    // `routine.toml` and `prompt.md` writes succeed, but `state.local.toml` is replaced
    // with a non-empty directory so the atomic rename over it fails.
    with_override_home(|_home| {
        let title = "Rs Runtime State Write Fail Routine";
        let slug = slugify(title);
        let mut routine = make_routine("rs-runtime-state-write-fail-id", title);
        routine.last_manual_trigger_at = Some(12345);

        // Block state.local.toml with a non-empty directory so the atomic rename fails.
        let state_path = crate::paths::routine_state_path(&slug);
        std::fs::create_dir_all(&state_path).unwrap();
        std::fs::write(state_path.join("occupant"), "block").unwrap();

        let result = write_routine(&routine);

        // Restore: remove blocking dir so with_override_home can clean up.
        std::fs::remove_dir_all(&state_path).unwrap();

        assert!(
            result.is_err(),
            "write_routine should fail when state sidecar cannot be written"
        );
    });
}

#[test]
fn write_runtime_state_fails_when_state_file_is_a_directory() {
    // Covers L210: `std::fs::remove_file(&path)?` — when `last_manual_trigger_at` is
    // `None` and the state path is a directory (not a regular file), `remove_file` fails
    // because it can only remove files, not directories.
    with_override_home(|_home| {
        let title = "Rs Remove State Dir Fail Routine";
        let slug = slugify(title);
        let mut routine = make_routine("rs-remove-state-dir-id", title);
        routine.last_manual_trigger_at = None;

        // Write once to create the slug dir and all regular sidecars.
        write_routine(&routine).unwrap();

        // Replace state.local.toml with a directory so remove_file fails.
        let state_path = crate::paths::routine_state_path(&slug);
        std::fs::create_dir_all(&state_path).unwrap();

        let result = write_routine(&routine);

        // Restore before assertions so with_override_home can clean up.
        std::fs::remove_dir_all(&state_path).unwrap();

        assert!(
            result.is_err(),
            "write_routine should fail when state.local.toml is a directory"
        );
    });
}

#[allow(
    clippy::missing_docs_in_private_items,
    reason = "split-out module keeps the file under the linecheck limit"
)]
#[path = "snooze_fields_round_trip_through_sidecar_not_routine_toml_tests.rs"]
mod snooze_fields_round_trip_through_sidecar_not_routine_toml_tests;
