
#[test]
fn migrate_prompts_to_subfolder_from_dir_skips_dir_without_routine_toml() {
    // An orphaned dir with no routine.toml at all (e.g. a leftover from a failed write) is not a
    // routine, so it is left untouched rather than getting an empty prompts/ sidecar resurrected.
    let dir = scratch_dir("prompts-subfolder-no-toml");
    std::fs::create_dir_all(&dir).unwrap();

    let orphan = dir.join("orphan-dir");
    std::fs::create_dir_all(&orphan).unwrap();

    migrate_prompts_to_subfolder_from_dir(&dir);

    assert!(!orphan.join("prompts").exists());

    std::fs::remove_dir_all(&dir).unwrap();
}

#[cfg(unix)]
#[test]
fn migrate_prompts_to_subfolder_from_dir_logs_on_create_dir_failure() {
    // A regular FILE occupies the `prompts` path, so `create_dir_all(prompts_dir)` fails and the
    // entry is skipped entirely (logged, `continue`).
    let dir = scratch_dir("prompts-subfolder-create-fail");
    std::fs::create_dir_all(&dir).unwrap();

    let routine = dir.join("blocked-routine");
    std::fs::create_dir_all(&routine).unwrap();
    std::fs::write(
        routine.join("routine.toml"),
        "title = \"Blocked\"\nschedule = \"@daily\"\nagent = \"claude\"\n",
    )
    .unwrap();
    std::fs::write(routine.join("prompts"), "i block the prompts dir").unwrap();

    migrate_prompts_to_subfolder_from_dir(&dir);

    assert!(
        routine.join("prompts").is_file(),
        "the blocking file is left in place"
    );

    std::fs::remove_dir_all(&dir).unwrap();
}

#[cfg(unix)]
#[test]
fn migrate_prompts_to_subfolder_from_dir_logs_on_rename_failure() {
    use std::os::unix::fs::PermissionsExt;

    // prompts/ already exists (writable), but the routine dir itself is read-only, so removing
    // the top-level prompt.md as part of the rename fails.
    let dir = scratch_dir("prompts-subfolder-rename-fail");
    std::fs::create_dir_all(&dir).unwrap();

    let routine = dir.join("rename-fail-routine");
    std::fs::create_dir_all(routine.join("prompts")).unwrap();
    std::fs::write(
        routine.join("routine.toml"),
        "title = \"Rename Fail\"\nschedule = \"@daily\"\nagent = \"claude\"\n",
    )
    .unwrap();
    std::fs::write(routine.join("prompt.md"), "old composed body").unwrap();
    std::fs::write(routine.join("prompts").join("prompt.pure.md"), "pure").unwrap();
    std::fs::set_permissions(&routine, std::fs::Permissions::from_mode(0o555)).unwrap();

    migrate_prompts_to_subfolder_from_dir(&dir);

    std::fs::set_permissions(&routine, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert!(
        routine.join("prompt.md").exists(),
        "the rename could not happen, so the old file remains"
    );
    assert!(!routine.join("prompts").join("prompt.compiled.md").exists());

    std::fs::remove_dir_all(&dir).unwrap();
}

#[allow(
    clippy::missing_docs_in_private_items,
    reason = "split-out module keeps the file under the linecheck limit"
)]
#[path = "migrate_prompts_to_subfolder_from_dir_logs_on_pure_write_failure_tests.rs"]
mod migrate_prompts_to_subfolder_from_dir_logs_on_pure_write_failure_tests;
