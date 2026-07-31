#![allow(
    clippy::wildcard_imports,
    clippy::too_many_arguments,
    clippy::needless_pass_by_value,
    dead_code,
    reason = "split-out module preserves existing code while keeping files under the linecheck limit"
)]
use super::*;

#[cfg(unix)]
#[test]
fn migrate_prompts_to_subfolder_from_dir_logs_on_pure_write_failure() {
    use std::os::unix::fs::PermissionsExt;

    // prompts/ exists but is read-only, so writing the extracted prompt.pure.md fails.
    let dir = scratch_dir("prompts-subfolder-write-fail");
    std::fs::create_dir_all(&dir).unwrap();

    let routine = dir.join("write-fail-routine");
    let prompts = routine.join("prompts");
    std::fs::create_dir_all(&prompts).unwrap();
    std::fs::write(
        routine.join("routine.toml"),
        "title = \"Write Fail\"\nschedule = \"@daily\"\nagent = \"claude\"\nprompt = \"raw\"\n",
    )
    .unwrap();
    std::fs::set_permissions(&prompts, std::fs::Permissions::from_mode(0o555)).unwrap();

    migrate_prompts_to_subfolder_from_dir(&dir);

    std::fs::set_permissions(&prompts, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert!(
        !prompts.join("prompt.pure.md").exists(),
        "the write could not happen"
    );

    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn migrate_prompts_to_subfolder_public_wrapper_runs() {
    // Exercises the public wrapper, which simply delegates to the inner variant scanning an empty
    // override home (no routines dir yet, so it returns without doing anything).
    with_override_home(|_home| {
        migrate_prompts_to_subfolder();
    });
}

#[test]
fn migrate_routine_dirs_from_dir_missing_dir_returns() {
    // The scan directory does not exist, so `read_dir` errors and the function returns early.
    let missing = scratch_dir("migrate-missing");
    migrate_routine_dirs_from_dir(&missing);
    assert!(!missing.exists());
}

#[test]
fn migrate_routine_dirs_from_dir_skips_non_dir_and_unparsable() {
    // With the home redirected, the inner variant scans the real (temp) routines dir, so
    // `load_routine_from_dir` resolves there too.
    with_override_home(|_home| {
        let routines = crate::paths::routines_dir();
        std::fs::create_dir_all(&routines).unwrap();

        // A plain file in the routines dir exercises the non-directory `continue` branch.
        std::fs::write(routines.join("stray.txt"), "ignore me").unwrap();

        // A directory whose routine.toml is unparsable exercises the unparsable-toml `continue`.
        let garbage = routines.join("garbage-dir");
        std::fs::create_dir_all(&garbage).unwrap();
        std::fs::write(garbage.join("routine.toml"), "id = \"x\"\nschedu").unwrap();

        migrate_routine_dirs_from_dir(&routines);

        // Neither entry was migrated away; both are left exactly as they were.
        assert!(routines.join("stray.txt").exists());
        assert!(garbage.join("routine.toml").exists());
    });
}

#[test]
fn migrate_routine_dirs_from_dir_skips_already_canonical_dir() {
    // A routine dir whose name already equals its slug needs no migration: the
    // `slug == dir_name` guard short-circuits with `continue`, leaving the dir untouched.
    with_override_home(|_home| {
        let routines = crate::paths::routines_dir();
        std::fs::create_dir_all(&routines).unwrap();

        // write_routine lays the routine down under its canonical slug-named dir.
        let title = "Rs Canonical Routine";
        let slug = slugify(title);
        write_routine(&make_routine("rs-canonical-id", title)).unwrap();
        // The on-disk dir name already equals the slug, so the scan hits the no-op guard.
        assert!(routines.join(&slug).is_dir());

        migrate_routine_dirs_from_dir(&routines);

        // Already canonical, so the dir stays exactly where it was.
        assert!(crate::paths::routine_toml_path(&slug).exists());
        assert_eq!(load_routine_from_dir(&slug).unwrap().id, "rs-canonical-id");
    });
}

#[test]
fn migrate_routine_dirs_from_dir_migrates_legacy_dir() {
    // The full happy path through the inner variant: a UUID-named legacy dir is re-persisted into its
    // slug dir and the legacy dir removed.
    with_override_home(|_home| {
        let routines = crate::paths::routines_dir();
        std::fs::create_dir_all(&routines).unwrap();

        let id = "rs-inner-legacy-uuid";
        let title = "Rs Inner Legacy Routine";
        let slug = slugify(title);
        let legacy_dir = crate::paths::routine_dir(id);
        std::fs::create_dir_all(&legacy_dir).unwrap();
        let toml = format!(
            "id = \"{id}\"\nschedule = \"@daily\"\ntitle = \"{title}\"\nagent = \"claude\"\nprompt = \"task\"\nenabled = true\n"
        );
        std::fs::write(legacy_dir.join("routine.toml"), toml).unwrap();
        std::fs::write(legacy_dir.join("prompt.md"), "legacy prompt").unwrap();

        migrate_routine_dirs_from_dir(&routines);

        assert!(!legacy_dir.exists(), "legacy UUID dir should be removed");
        assert!(crate::paths::routine_toml_path(&slug).exists());
        let loaded = load_routine_from_dir(&slug).unwrap();
        assert_eq!(loaded.id, id, "UUID id preserved across the dir migration");
    });
}

#[cfg(unix)]
#[test]
fn migrate_routine_dirs_from_dir_logs_on_remove_failure() {
    // write_routine succeeds (the slug dir is created in the writable routines dir), but removing the
    // legacy dir fails: the legacy dir is made read-only, so deleting its contents is denied. This
    // exercises the remove-failure `log::warn!` branch.
    use std::os::unix::fs::PermissionsExt;
    with_override_home(|_home| {
        let routines = crate::paths::routines_dir();
        std::fs::create_dir_all(&routines).unwrap();

        let id = "rs-remove-fail-uuid";
        let title = "Rs Remove Fail Routine";
        let slug = slugify(title);
        let legacy_dir = crate::paths::routine_dir(id);
        std::fs::create_dir_all(&legacy_dir).unwrap();
        let toml = format!(
            "id = \"{id}\"\nschedule = \"@daily\"\ntitle = \"{title}\"\nagent = \"claude\"\nprompt = \"task\"\nenabled = true\n"
        );
        std::fs::write(legacy_dir.join("routine.toml"), &toml).unwrap();
        std::fs::write(legacy_dir.join("schedule.cron"), "@daily\n").unwrap();
        std::fs::write(legacy_dir.join("prompt.md"), "legacy").unwrap();

        // Read-only legacy dir blocks removing its own children, so remove_dir_all fails.
        std::fs::set_permissions(&legacy_dir, std::fs::Permissions::from_mode(0o555)).unwrap();

        migrate_routine_dirs_from_dir(&routines);

        // The write into the slug dir succeeded.
        assert!(crate::paths::routine_toml_path(&slug).exists());
        // Restore permissions so the legacy dir can be inspected and cleaned up; it survives because
        // the removal failed and was only logged.
        std::fs::set_permissions(&legacy_dir, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert!(legacy_dir.exists(), "legacy dir survives a failed removal");
    });
}
include!("routine_storage_migration_tests_part4.rs");
