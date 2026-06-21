#![allow(clippy::missing_docs_in_private_items)]

use super::*;
use crate::routines::{slugify, Repository, Routine};

fn make_routine(id: &str, title: &str) -> Routine {
    Routine {
        id: id.to_string(),
        schedule: "@daily".to_string(),
        title: title.to_string(),
        agent: "claude".to_string(),
        prompt: "task".to_string(),
        repositories: vec![Repository {
            repository: "https://example.com/r.git".to_string(),
            branch: Some("main".to_string()),
        }],
        enabled: true,
        source: "managed".to_string(),
        created_at: 5,
        updated_at: 6,
        last_manual_trigger_at: None,
        last_scheduled_trigger_at: None,
        ttl_secs: None,
        max_runtime_secs: None,
    }
}

#[test]
fn load_store_from_dir_inserts_written_routines() {
    // Covers the `routines.insert(..)` arm of `load_store_from_dir`: a directory holding a valid
    // routine sub-folder is scanned and the parsed routine lands in the returned store.
    with_override_home(|_home| {
        write_routine(&make_routine("rs-loadstore-id", "Rs Loadstore Routine")).unwrap();
        // A stray non-directory entry alongside the routine folder exercises the `is_dir == false`
        // skip path of the scan loop.
        std::fs::write(crate::paths::routines_dir().join("stray.txt"), b"x").unwrap();
        let store = load_store_from_dir(&crate::paths::routines_dir());
        assert!(store
            .lock()
            .unwrap()
            .values()
            .any(|routine| routine.id == "rs-loadstore-id"));
    });
}

#[test]
fn write_then_load_round_trips() {
    let id = "rs-roundtrip-id";
    let title = "Rs Roundtrip Routine";
    let slug = slugify(title);
    let routine = make_routine(id, title);
    write_routine(&routine).unwrap();

    assert!(crate::paths::routine_toml_path(&slug).exists());
    assert!(crate::paths::routine_prompt_path(&slug).exists());
    assert!(crate::paths::routine_gitignore_path(&slug).exists());

    let loaded = load_routine_from_dir(&slug).unwrap();
    assert_eq!(loaded.id, id);
    assert_eq!(loaded.schedule, "@daily");
    assert_eq!(loaded.title, title);
    assert_eq!(loaded.agent, "claude");
    assert_eq!(loaded.prompt, "task");
    assert_eq!(loaded.repositories.len(), 1);
    assert_eq!(loaded.repositories[0].branch.as_deref(), Some("main"));
    assert!(loaded.enabled);

    remove_routine_dir(&slug).unwrap();
    assert!(!crate::paths::routine_dir(&slug).exists());
}

#[test]
fn prompt_file_contains_composed_prompt() {
    let title = "Rs Prompt Routine";
    let slug = slugify(title);
    write_routine(&make_routine("rs-prompt-id", title)).unwrap();
    let prompt = std::fs::read_to_string(crate::paths::routine_prompt_path(&slug)).unwrap();
    assert!(prompt.contains("# Workbench"));
    assert!(prompt.contains("https://example.com/r.git (branch main)"));
    assert!(prompt.contains("task"));
    remove_routine_dir(&slug).unwrap();
}

#[test]
fn write_routine_persists_composed_prompt_sidecar_with_repos() {
    // Focused coverage for the `atomic_write(routine_prompt_path, compose_prompt(..))`
    // call in `write_routine`: a routine with a non-empty prompt AND repositories runs
    // `compose_prompt` fully, and the composed body lands in prompt.md on disk.
    let id = "rs-prompt-sidecar-id";
    let title = "Rs Prompt Sidecar Routine";
    let slug = slugify(title);
    let mut routine = make_routine(id, title);
    routine.prompt = "line one\nline two".to_string();
    routine.repositories = vec![
        Repository {
            repository: "https://example.com/a.git".to_string(),
            branch: Some("dev".to_string()),
        },
        Repository {
            repository: "https://example.com/b.git".to_string(),
            branch: None,
        },
    ];

    write_routine(&routine).unwrap();

    let written = std::fs::read_to_string(crate::paths::routine_prompt_path(&slug)).unwrap();
    assert_eq!(written, compose_prompt(&routine));
    assert!(written.contains("https://example.com/a.git (branch dev)"));
    assert!(written.contains("https://example.com/b.git\n"));
    assert!(written.contains("line one\nline two"));

    remove_routine_dir(&slug).unwrap();
}

#[test]
fn write_routine_errors_when_prompt_sidecar_write_fails() {
    // Covers the error-propagation (`?`) on the prompt `atomic_write` in `write_routine`:
    // the routine dir, gitignore, and `routine.toml` all write successfully, but a
    // non-empty directory occupies the `prompt.md` path, so the atomic rename over it
    // fails and `write_routine` returns that error.
    let id = "rs-prompt-write-fail-id";
    let title = "Rs Prompt Write Fail Routine";
    let slug = slugify(title);
    let dir = crate::paths::routine_dir(&slug);
    std::fs::create_dir_all(&dir).unwrap();
    // Block prompt.md with a *non-empty* directory so the atomic rename over it fails.
    let prompt_dir = crate::paths::routine_prompt_path(&slug);
    std::fs::create_dir_all(&prompt_dir).unwrap();
    std::fs::write(prompt_dir.join("occupant"), "keep me non-empty").unwrap();

    let err = write_routine(&make_routine(id, title)).unwrap_err();
    let _ = err;

    // routine.toml was written successfully before the prompt step failed.
    assert!(crate::paths::routine_toml_path(&slug).exists());
    assert!(
        prompt_dir.is_dir(),
        "the blocking prompt dir is left in place"
    );

    remove_routine_dir(&slug).unwrap();
}

#[test]
fn load_routine_from_dir_applies_defaults_for_absent_optional_fields() {
    // A minimal routine.toml that omits prompt, enabled, timestamps, and id exercises the
    // default-fallback arms in load_routine_from_dir: prompt -> "", enabled -> true,
    // created_at/updated_at -> 0, and id -> dir_name (legacy fallback).
    let slug = "rs-defaults-routine";
    let dir = crate::paths::routine_dir(slug);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        crate::paths::routine_toml_path(slug),
        "schedule = \"@daily\"\ntitle = \"Rs Defaults Routine\"\nagent = \"claude\"\n",
    )
    .unwrap();

    let loaded = load_routine_from_dir(slug).unwrap();
    assert_eq!(loaded.id, slug, "absent id falls back to the dir name");
    assert_eq!(loaded.prompt, "", "absent prompt defaults to empty");
    assert!(loaded.enabled, "absent enabled defaults to true");
    assert_eq!(loaded.created_at, 0);
    assert_eq!(loaded.updated_at, 0);
    assert!(loaded.repositories.is_empty());

    remove_routine_dir(slug).unwrap();
}

#[test]
fn load_routine_from_dir_missing_returns_none() {
    assert!(load_routine_from_dir("rs-does-not-exist-zzz").is_none());
}

#[test]
fn last_manual_trigger_at_persists_to_sidecar_not_routine_toml() {
    // Runtime trigger state is written to the gitignored `state.local.toml` sidecar and kept out
    // of the version-controlled `routine.toml`, then read back from the sidecar on load.
    let title = "Rs Sidecar Routine";
    let slug = slugify(title);
    let mut routine = make_routine("rs-sidecar-id", title);
    routine.last_manual_trigger_at = Some(12345);
    write_routine(&routine).unwrap();

    // The tracked config file does not carry the runtime timestamp...
    let toml_text = std::fs::read_to_string(crate::paths::routine_toml_path(&slug)).unwrap();
    assert!(
        !toml_text.contains("last_manual_trigger_at"),
        "routine.toml must not carry runtime trigger state: {toml_text}"
    );
    // ...the gitignored sidecar does, and it round-trips through load.
    assert!(crate::paths::routine_state_path(&slug).exists());
    let state_text = std::fs::read_to_string(crate::paths::routine_state_path(&slug)).unwrap();
    assert!(state_text.contains("last_manual_trigger_at"));
    assert_eq!(
        load_routine_from_dir(&slug).unwrap().last_manual_trigger_at,
        Some(12345)
    );

    remove_routine_dir(&slug).unwrap();
}

#[test]
fn write_routine_clears_stale_sidecar_when_untriggered() {
    // Re-writing a routine whose trigger state has been cleared removes the now-stale sidecar, so
    // the on-disk state mirrors the in-memory `None`.
    let title = "Rs Clear Sidecar Routine";
    let slug = slugify(title);
    let mut routine = make_routine("rs-clear-id", title);
    routine.last_manual_trigger_at = Some(999);
    write_routine(&routine).unwrap();
    assert!(crate::paths::routine_state_path(&slug).exists());

    routine.last_manual_trigger_at = None;
    write_routine(&routine).unwrap();
    assert!(
        !crate::paths::routine_state_path(&slug).exists(),
        "sidecar should be removed when there is no trigger state"
    );
    assert_eq!(
        load_routine_from_dir(&slug).unwrap().last_manual_trigger_at,
        None
    );

    remove_routine_dir(&slug).unwrap();
}

#[test]
fn load_routine_falls_back_to_legacy_last_triggered_in_routine_toml() {
    // A routine written by an older daemon stored `last_triggered_at` inside `routine.toml` and
    // has no sidecar. Load still surfaces the timestamp via the legacy-field fallback.
    let slug = "rs-legacy-trigger-routine";
    let dir = crate::paths::routine_dir(slug);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        crate::paths::routine_toml_path(slug),
        "schedule = \"@daily\"\ntitle = \"Rs Legacy Trigger\"\nagent = \"claude\"\nlast_triggered_at = 777\n",
    )
    .unwrap();
    // No sidecar exists yet.
    assert!(!crate::paths::routine_state_path(slug).exists());

    assert_eq!(
        load_routine_from_dir(slug).unwrap().last_manual_trigger_at,
        Some(777)
    );

    remove_routine_dir(slug).unwrap();
}

#[test]
fn load_routine_ignores_unparsable_sidecar() {
    // A malformed `state.local.toml` parses to `None` (rather than crashing the load), and with no
    // legacy field in `routine.toml` the routine loads with no trigger timestamp.
    let slug = "rs-bad-sidecar-routine";
    let dir = crate::paths::routine_dir(slug);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        crate::paths::routine_toml_path(slug),
        "schedule = \"@daily\"\ntitle = \"Rs Bad Sidecar\"\nagent = \"claude\"\n",
    )
    .unwrap();
    std::fs::write(crate::paths::routine_state_path(slug), "= not valid toml =").unwrap();

    assert_eq!(
        load_routine_from_dir(slug).unwrap().last_manual_trigger_at,
        None
    );

    remove_routine_dir(slug).unwrap();
}

#[test]
fn load_routine_reads_scheduled_trigger_from_sidecar() {
    // `last_scheduled_trigger_at` lives in its own gitignored `scheduled.local.toml` sidecar,
    // written by the routine's `run.sh` at cron fire time, and is read back on load — independently
    // of the manual-trigger sidecar.
    let title = "Rs Scheduled Sidecar Routine";
    let slug = slugify(title);
    write_routine(&make_routine("rs-scheduled-id", title)).unwrap();
    std::fs::write(
        crate::paths::routine_scheduled_state_path(&slug),
        "last_scheduled_trigger_at = 4242\n",
    )
    .unwrap();

    let loaded = load_routine_from_dir(&slug).unwrap();
    assert_eq!(loaded.last_scheduled_trigger_at, Some(4242));
    // The scheduled timestamp is distinct from the (unset) manual one.
    assert_eq!(loaded.last_manual_trigger_at, None);

    remove_routine_dir(&slug).unwrap();
}

#[test]
fn load_routine_ignores_unparsable_scheduled_sidecar() {
    // A malformed `scheduled.local.toml` parses to `None` rather than crashing the load.
    let slug = "rs-bad-scheduled-sidecar-routine";
    let dir = crate::paths::routine_dir(slug);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        crate::paths::routine_toml_path(slug),
        "schedule = \"@daily\"\ntitle = \"Rs Bad Scheduled Sidecar\"\nagent = \"claude\"\n",
    )
    .unwrap();
    std::fs::write(
        crate::paths::routine_scheduled_state_path(slug),
        "= not valid toml =",
    )
    .unwrap();

    assert_eq!(
        load_routine_from_dir(slug)
            .unwrap()
            .last_scheduled_trigger_at,
        None
    );

    remove_routine_dir(slug).unwrap();
}

#[test]
fn write_routine_preserves_scheduler_written_scheduled_sidecar() {
    // The daemon never writes the scheduled sidecar, so re-persisting a routine (e.g. on startup or
    // an update) must leave the scheduler-stamped `scheduled.local.toml` untouched — the bug this
    // separate-file design exists to prevent.
    let title = "Rs Preserve Scheduled Routine";
    let slug = slugify(title);
    let mut routine = make_routine("rs-preserve-scheduled-id", title);
    write_routine(&routine).unwrap();

    // Simulate a scheduled cron firing stamping the sidecar.
    std::fs::write(
        crate::paths::routine_scheduled_state_path(&slug),
        "last_scheduled_trigger_at = 55\n",
    )
    .unwrap();

    // A subsequent daemon-side write (manual trigger recorded, routine updated, repersist, …).
    routine.last_manual_trigger_at = Some(7);
    write_routine(&routine).unwrap();

    assert!(
        crate::paths::routine_scheduled_state_path(&slug).exists(),
        "daemon write must not remove the scheduler-owned sidecar"
    );
    let loaded = load_routine_from_dir(&slug).unwrap();
    assert_eq!(loaded.last_scheduled_trigger_at, Some(55));
    assert_eq!(loaded.last_manual_trigger_at, Some(7));

    remove_routine_dir(&slug).unwrap();
}

#[test]
fn torn_routine_toml_loads_as_none() {
    // A truncated/garbage routine.toml (e.g. left by a crash mid-write) must not panic or load a
    // half-baked routine; the loader returns None and the routine is simply absent.
    let slug = "rs-torn-toml-routine";
    let dir = crate::paths::routine_dir(slug);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(crate::paths::routine_toml_path(slug), "id = \"x\"\nschedu").unwrap();
    assert!(load_routine_from_dir(slug).is_none());
    remove_routine_dir(slug).unwrap();
}

#[test]
fn write_routine_leaves_no_tmp_residue() {
    let id = "rs-no-residue-id";
    let title = "Rs No Residue Routine";
    let slug = slugify(title);
    write_routine(&make_routine(id, title)).unwrap();
    let residue = std::fs::read_dir(crate::paths::routine_dir(&slug))
        .unwrap()
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_name().to_string_lossy().contains(".tmp"))
        .count();
    assert_eq!(residue, 0, "atomic_write must leave no .tmp files behind");
    remove_routine_dir(&slug).unwrap();
}

#[test]
fn load_store_includes_written_routine() {
    let id = "rs-loadstore-id";
    let title = "Rs Loadstore Routine";
    let slug = slugify(title);
    write_routine(&make_routine(id, title)).unwrap();
    let store = load_store();
    assert!(store.lock().unwrap().contains_key(id));
    remove_routine_dir(&slug).unwrap();
}

#[test]
fn load_store_from_dir_skips_unloadable_dirs() {
    // Covers both `None` arms of the scan loop: a dir whose routine.toml is present but
    // unparsable (warned + skipped) and a dir with no routine.toml at all (skipped quietly).
    // Neither lands in the store, and a valid sibling routine is unaffected.
    with_override_home(|_home| {
        write_routine(&make_routine("rs-valid-id", "Rs Valid Routine")).unwrap();

        let bad_dir = crate::paths::routine_dir("rs-bad-toml");
        std::fs::create_dir_all(&bad_dir).unwrap();
        std::fs::write(
            crate::paths::routine_toml_path("rs-bad-toml"),
            "id = \"x\"\nschedu",
        )
        .unwrap();

        let empty_dir = crate::paths::routine_dir("rs-no-toml");
        std::fs::create_dir_all(&empty_dir).unwrap();

        let store = load_store_from_dir(&crate::paths::routines_dir());
        let guard = store.lock().unwrap();
        assert!(guard.values().any(|routine| routine.id == "rs-valid-id"));
        assert!(!guard.contains_key("rs-bad-toml"));
        assert!(!guard.contains_key("rs-no-toml"));
    });
}

#[test]
fn load_store_from_dir_missing_dir_empty() {
    let store = load_store_from_dir(std::path::Path::new("/nonexistent-routines-dir-99999"));
    assert!(store.lock().unwrap().is_empty());
}

#[test]
fn remove_routine_dir_noop_when_absent() {
    remove_routine_dir("rs-never-created-zzz").unwrap();
}

#[test]
fn migrate_routine_dirs_moves_legacy_uuid_dir_to_slug() {
    let id = "rs-legacy-uuid-1234";
    let title = "Rs Legacy Migrate Routine";
    let slug = slugify(title);
    let legacy_dir = crate::paths::routine_dir(id);
    std::fs::create_dir_all(&legacy_dir).unwrap();
    // Legacy layout: routine.toml + prompt.md live under the UUID-named dir.
    let toml = format!(
        "id = \"{id}\"\nschedule = \"@daily\"\ntitle = \"{title}\"\nagent = \"claude\"\nprompt = \"task\"\nenabled = true\n"
    );
    std::fs::write(legacy_dir.join("routine.toml"), toml).unwrap();
    std::fs::write(legacy_dir.join("prompt.md"), "legacy prompt").unwrap();

    migrate_routine_dirs();

    // Legacy dir removed; canonical slug dir now holds toml + prompt.
    assert!(!legacy_dir.exists(), "legacy UUID dir should be removed");
    assert!(crate::paths::routine_toml_path(&slug).exists());
    assert!(crate::paths::routine_prompt_path(&slug).exists());
    let loaded = load_routine_from_dir(&slug).unwrap();
    assert_eq!(loaded.id, id, "UUID id preserved across the dir migration");

    remove_routine_dir(&slug).unwrap();
}

#[test]
fn repersist_routines_recreates_missing_prompt_sidecar() {
    let id = "rs-repersist-id";
    let title = "Rs Repersist Routine";
    let slug = slugify(title);
    write_routine(&make_routine(id, title)).unwrap();
    // Simulate the sync-only state: prompt.md gone, only run.sh-style dir remains.
    std::fs::remove_file(crate::paths::routine_prompt_path(&slug)).unwrap();
    assert!(!crate::paths::routine_prompt_path(&slug).exists());

    let mut map = HashMap::new();
    map.insert(id.to_string(), make_routine(id, title));
    let store = Arc::new(Mutex::new(map));
    repersist_routines(&store);

    assert!(
        crate::paths::routine_prompt_path(&slug).exists(),
        "repersist should recreate the prompt sidecar"
    );
    remove_routine_dir(&slug).unwrap();
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

#[test]
fn repersist_routines_logs_on_write_failure() {
    // A routine whose slug directory path is occupied by a regular file makes write_routine fail,
    // exercising the `log::warn!` failure branch in repersist_routines.
    with_override_home(|_home| {
        let routines = crate::paths::routines_dir();
        std::fs::create_dir_all(&routines).unwrap();

        let id = "rs-repersist-fail-id";
        let title = "Rs Repersist Fail Routine";
        let slug = slugify(title);
        // Block the slug dir with a regular file so create_dir_all fails inside write_routine.
        std::fs::write(routines.join(&slug), "block").unwrap();

        let mut map = HashMap::new();
        map.insert(id.to_string(), make_routine(id, title));
        let store = Arc::new(Mutex::new(map));
        repersist_routines(&store);

        // The write failed and was only logged; the blocking file remains.
        assert!(routines.join(&slug).is_file());
    });
}
