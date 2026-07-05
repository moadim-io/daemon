#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use super::*;
use crate::routines::{slugify, Routine};

fn make_routine(id: &str, title: &str) -> Routine {
    Routine {
        model: None,
        id: id.to_string(),
        schedule: "@daily".to_string(),
        title: title.to_string(),
        agent: "claude".to_string(),
        prompt: "do the thing".to_string(),
        goal: None,
        repositories: vec![],
        machines: vec![],
        enabled: true,
        source: "managed".to_string(),
        created_at: 0,
        updated_at: 0,
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

/// A unique, not-yet-created scratch directory under the system temp dir.
fn scratch_dir(tag: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("moadim-rs-{tag}-{}", uuid::Uuid::new_v4()))
}

/// Run `body` with `MOADIM_HOME_OVERRIDE` pointed at a fresh temp home, restoring the previous value
/// and removing the temp home afterwards. Mirrors the seam used by the agent registry tests.
fn with_override_home(body: impl FnOnce(&std::path::Path)) {
    let home = scratch_dir("override-home");
    std::fs::create_dir_all(&home).unwrap();
    let previous = std::env::var_os("MOADIM_HOME_OVERRIDE");
    // SAFETY: tests in this crate run single-threaded per binary; we set and immediately restore the
    // override around this call.
    unsafe {
        std::env::set_var("MOADIM_HOME_OVERRIDE", &home);
    }
    body(&home);
    unsafe {
        match previous {
            Some(value) => std::env::set_var("MOADIM_HOME_OVERRIDE", value),
            None => std::env::remove_var("MOADIM_HOME_OVERRIDE"),
        }
    }
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn migrate_prompt_files_from_dir_missing_dir_returns() {
    // The scan directory does not exist, so `read_dir` errors and the function returns early.
    let missing = scratch_dir("prompt-missing");
    migrate_prompt_files_from_dir(&missing);
    assert!(!missing.exists());
}

#[test]
fn migrate_prompt_files_from_dir_renames_txt_and_skips_non_dirs_and_existing() {
    let dir = scratch_dir("prompt-rename");
    std::fs::create_dir_all(&dir).unwrap();

    // A plain file in the scan dir exercises the non-directory `continue` branch.
    std::fs::write(dir.join("loose.txt"), "ignore me").unwrap();

    // A routine dir with only `prompt.txt`: it should be renamed to `prompt.md`.
    let renameable = dir.join("renameable");
    std::fs::create_dir_all(&renameable).unwrap();
    std::fs::write(renameable.join("prompt.txt"), "old body").unwrap();

    // A routine dir that already has `prompt.md`: the rename is skipped, leaving both files intact.
    let already = dir.join("already");
    std::fs::create_dir_all(&already).unwrap();
    std::fs::write(already.join("prompt.txt"), "stale").unwrap();
    std::fs::write(already.join("prompt.md"), "current").unwrap();

    migrate_prompt_files_from_dir(&dir);

    assert!(!renameable.join("prompt.txt").exists());
    assert_eq!(
        std::fs::read_to_string(renameable.join("prompt.md")).unwrap(),
        "old body"
    );
    // Pre-existing prompt.md is untouched; the stale prompt.txt stays put.
    assert!(already.join("prompt.txt").exists());
    assert_eq!(
        std::fs::read_to_string(already.join("prompt.md")).unwrap(),
        "current"
    );

    std::fs::remove_dir_all(&dir).unwrap();
}

#[cfg(unix)]
#[test]
fn migrate_prompt_files_from_dir_logs_on_rename_failure() {
    use std::os::unix::fs::PermissionsExt;

    // A routine dir holding `prompt.txt` but made read-only: renaming within it fails because the
    // directory cannot be modified, exercising the `log::warn!` rename-failure branch.
    let dir = scratch_dir("prompt-rename-fail");
    std::fs::create_dir_all(&dir).unwrap();
    let locked = dir.join("locked");
    std::fs::create_dir_all(&locked).unwrap();
    std::fs::write(locked.join("prompt.txt"), "body").unwrap();
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o555)).unwrap();

    migrate_prompt_files_from_dir(&dir);

    // The rename could not happen: prompt.txt remains and prompt.md was never created.
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert!(locked.join("prompt.txt").exists());
    assert!(!locked.join("prompt.md").exists());

    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn migrate_prompt_files_public_wrapper_runs() {
    // Exercises the public wrapper, which simply delegates to the inner variant scanning an empty
    // override home (no routines dir yet, so it returns without doing anything).
    with_override_home(|_home| {
        migrate_prompt_files();
    });
}

#[test]
fn migrate_prompts_to_subfolder_from_dir_missing_dir_returns() {
    // The scan directory does not exist, so `read_dir` errors and the function returns early.
    let missing = scratch_dir("prompts-subfolder-missing");
    migrate_prompts_to_subfolder_from_dir(&missing);
    assert!(!missing.exists());
}

#[test]
fn migrate_prompts_to_subfolder_from_dir_migrates_legacy_layout() {
    let dir = scratch_dir("prompts-subfolder-migrate");
    std::fs::create_dir_all(&dir).unwrap();

    // A plain file in the scan dir exercises the non-directory `continue` branch.
    std::fs::write(dir.join("loose.txt"), "ignore me").unwrap();

    // A legacy routine dir: top-level prompt.md (composed) + routine.toml carrying the raw
    // prompt in its (legacy) `prompt` field, no `prompts/` subfolder yet.
    let legacy = dir.join("legacy-routine");
    std::fs::create_dir_all(&legacy).unwrap();
    std::fs::write(legacy.join("prompt.md"), "old composed body").unwrap();
    std::fs::write(
        legacy.join("routine.toml"),
        "title = \"Legacy\"\nschedule = \"@daily\"\nagent = \"claude\"\nprompt = \"raw prompt\"\n",
    )
    .unwrap();

    migrate_prompts_to_subfolder_from_dir(&dir);

    assert!(
        !legacy.join("prompt.md").exists(),
        "top-level prompt.md should be moved"
    );
    assert_eq!(
        std::fs::read_to_string(legacy.join("prompts").join("prompt.compiled.md")).unwrap(),
        "old composed body"
    );
    assert_eq!(
        std::fs::read_to_string(legacy.join("prompts").join("prompt.pure.md")).unwrap(),
        "raw prompt"
    );

    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn migrate_prompts_to_subfolder_from_dir_skips_already_migrated() {
    // A dir already in the new layout (both prompts/ files present, no top-level prompt.md) is
    // left untouched: the `!new_compiled.exists()` and `!pure.exists()` guards both short-circuit.
    let dir = scratch_dir("prompts-subfolder-skip");
    std::fs::create_dir_all(&dir).unwrap();

    let routine = dir.join("already-migrated");
    let prompts = routine.join("prompts");
    std::fs::create_dir_all(&prompts).unwrap();
    std::fs::write(prompts.join("prompt.compiled.md"), "compiled").unwrap();
    std::fs::write(prompts.join("prompt.pure.md"), "pure").unwrap();
    std::fs::write(
        routine.join("routine.toml"),
        "title = \"Already\"\nschedule = \"@daily\"\nagent = \"claude\"\n",
    )
    .unwrap();

    migrate_prompts_to_subfolder_from_dir(&dir);

    assert_eq!(
        std::fs::read_to_string(prompts.join("prompt.compiled.md")).unwrap(),
        "compiled"
    );
    assert_eq!(
        std::fs::read_to_string(prompts.join("prompt.pure.md")).unwrap(),
        "pure"
    );

    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn migrate_prompts_to_subfolder_from_dir_defaults_missing_legacy_prompt_to_empty() {
    // A routine dir with a routine.toml but no `prompt` field (and no prompts/ subfolder yet)
    // still gets an (empty) prompt.pure.md written.
    let dir = scratch_dir("prompts-subfolder-no-legacy");
    std::fs::create_dir_all(&dir).unwrap();

    let routine = dir.join("no-legacy-prompt");
    std::fs::create_dir_all(&routine).unwrap();
    std::fs::write(
        routine.join("routine.toml"),
        "title = \"No Legacy\"\nschedule = \"@daily\"\nagent = \"claude\"\n",
    )
    .unwrap();

    migrate_prompts_to_subfolder_from_dir(&dir);

    assert_eq!(
        std::fs::read_to_string(routine.join("prompts").join("prompt.pure.md")).unwrap(),
        ""
    );

    std::fs::remove_dir_all(&dir).unwrap();
}

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

#[test]
fn migrate_routine_dirs_from_dir_logs_on_write_failure() {
    // write_routine fails when a regular FILE occupies the slug directory path, so `create_dir_all`
    // for that slug errors. The function logs and continues, leaving the legacy dir untouched.
    with_override_home(|_home| {
        let routines = crate::paths::routines_dir();
        std::fs::create_dir_all(&routines).unwrap();

        let id = "rs-write-fail-uuid";
        let title = "Rs Write Fail Routine";
        let slug = slugify(title);
        let legacy_dir = crate::paths::routine_dir(id);
        std::fs::create_dir_all(&legacy_dir).unwrap();
        let toml = format!(
            "id = \"{id}\"\nschedule = \"@daily\"\ntitle = \"{title}\"\nagent = \"claude\"\nprompt = \"task\"\nenabled = true\n"
        );
        std::fs::write(legacy_dir.join("routine.toml"), toml).unwrap();
        std::fs::write(legacy_dir.join("prompt.md"), "legacy").unwrap();

        // Place a regular file where the slug directory should go, so create_dir_all(&slug) fails.
        std::fs::write(routines.join(&slug), "i block the slug dir").unwrap();

        migrate_routine_dirs_from_dir(&routines);

        // The write failed, so the legacy dir is preserved and the slug path is still the file.
        assert!(legacy_dir.exists(), "legacy dir is left when write fails");
        assert!(routines.join(&slug).is_file());
    });
}

#[test]
fn migrate_routine_dirs_public_wrapper_runs() {
    // Exercises the public wrapper delegating into an empty override home.
    with_override_home(|_home| {
        migrate_routine_dirs();
    });
}
