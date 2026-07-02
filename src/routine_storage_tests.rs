#![allow(clippy::missing_docs_in_private_items)]

use super::*;
use crate::routines::{slugify, Repository, Routine};

fn make_routine(id: &str, title: &str) -> Routine {
    Routine {
        model: None,
        id: id.to_string(),
        schedule: "@daily".to_string(),
        title: title.to_string(),
        agent: "claude".to_string(),
        prompt: "task".to_string(),
        goal: None,
        repositories: vec![Repository {
            repository: "https://example.com/r.git".to_string(),
            branch: Some("main".to_string()),
        }],
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
    with_override_home(|_home| {
        let id = "rs-roundtrip-id";
        let title = "Rs Roundtrip Routine";
        let slug = slugify(title);
        let routine = make_routine(id, title);
        write_routine(&routine).unwrap();

        assert!(crate::paths::routine_toml_path(&slug).exists());
        assert!(crate::paths::routine_pure_prompt_path(&slug).exists());
        assert!(crate::paths::routine_compiled_prompt_path(&slug).exists());
        assert!(crate::paths::routine_gitignore_path(&slug).exists());
        let toml_text = std::fs::read_to_string(crate::paths::routine_toml_path(&slug)).unwrap();
        assert!(
            !toml_text.contains("prompt"),
            "routine.toml must not carry the prompt: {toml_text}"
        );
        assert_eq!(
            std::fs::read_to_string(crate::paths::routine_pure_prompt_path(&slug)).unwrap(),
            "task"
        );

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
    });
}

#[test]
fn tags_round_trip_through_routine_toml() {
    // Tags are persisted to the tracked `routine.toml` and read back on load.
    let title = "Rs Tags Routine";
    let slug = slugify(title);
    let mut routine = make_routine("rs-tags-id", title);
    routine.tags = vec!["triage".to_string(), "nightly".to_string()];
    write_routine(&routine).unwrap();

    let toml_text = std::fs::read_to_string(crate::paths::routine_toml_path(&slug)).unwrap();
    assert!(toml_text.contains("tags"), "routine.toml should carry tags");

    let loaded = load_routine_from_dir(&slug).unwrap();
    assert_eq!(
        loaded.tags,
        vec!["triage".to_string(), "nightly".to_string()]
    );

    remove_routine_dir(&slug).unwrap();
}

#[test]
fn prompt_file_contains_composed_prompt() {
    with_override_home(|_home| {
        let title = "Rs Prompt Routine";
        let slug = slugify(title);
        write_routine(&make_routine("rs-prompt-id", title)).unwrap();
        let prompt =
            std::fs::read_to_string(crate::paths::routine_compiled_prompt_path(&slug)).unwrap();
        assert!(prompt.contains("# Workbench"));
        assert!(prompt.contains("https://example.com/r.git (branch main)"));
        assert!(prompt.contains("task"));
    });
}

#[test]
fn write_routine_persists_composed_prompt_sidecar_with_repos() {
    // Focused coverage for the `atomic_write(routine_compiled_prompt_path, compose_prompt(..))`
    // call in `write_routine`: a routine with a non-empty prompt AND repositories runs
    // `compose_prompt` fully, and the composed body lands in prompts/prompt.compiled.md on disk.
    with_override_home(|_home| {
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

        let written =
            std::fs::read_to_string(crate::paths::routine_compiled_prompt_path(&slug)).unwrap();
        assert_eq!(written, compose_prompt(&routine));
        assert!(written.contains("https://example.com/a.git (branch dev)"));
        assert!(written.contains("https://example.com/b.git\n"));
        assert!(written.contains("line one\nline two"));

        let pure = std::fs::read_to_string(crate::paths::routine_pure_prompt_path(&slug)).unwrap();
        assert_eq!(pure, "line one\nline two");
    });
}

#[test]
fn write_routine_errors_when_prompt_sidecar_write_fails() {
    // Covers the error-propagation (`?`) on the pure-prompt `atomic_write` in `write_routine`:
    // the routine dir, gitignore, and `routine.toml` all write successfully, but a
    // non-empty directory occupies the `prompt.pure.md` path, so the atomic rename over it
    // fails and `write_routine` returns that error.
    with_override_home(|_home| {
        let id = "rs-prompt-write-fail-id";
        let title = "Rs Prompt Write Fail Routine";
        let slug = slugify(title);
        let dir = crate::paths::routine_dir(&slug);
        std::fs::create_dir_all(&dir).unwrap();
        // Block prompt.pure.md with a *non-empty* directory so the atomic rename over it fails.
        let prompt_dir = crate::paths::routine_pure_prompt_path(&slug);
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
    });
}

#[test]
fn write_routine_errors_when_compiled_prompt_sidecar_write_fails() {
    // Covers the error-propagation (`?`) on the compiled-prompt `atomic_write` in `write_routine`:
    // routine.toml and the pure-prompt sidecar both write successfully, but a non-empty directory
    // occupies the `prompt.compiled.md` path, so the atomic rename over it fails.
    with_override_home(|_home| {
        let id = "rs-compiled-prompt-write-fail-id";
        let title = "Rs Compiled Prompt Write Fail Routine";
        let slug = slugify(title);
        let dir = crate::paths::routine_dir(&slug);
        std::fs::create_dir_all(&dir).unwrap();
        // Block prompt.compiled.md with a *non-empty* directory so the atomic rename over it fails.
        let prompt_dir = crate::paths::routine_compiled_prompt_path(&slug);
        std::fs::create_dir_all(&prompt_dir).unwrap();
        std::fs::write(prompt_dir.join("occupant"), "keep me non-empty").unwrap();

        let err = write_routine(&make_routine(id, title)).unwrap_err();
        let _ = err;

        // routine.toml and the pure prompt were both written successfully before this step failed.
        assert!(crate::paths::routine_toml_path(&slug).exists());
        assert!(crate::paths::routine_pure_prompt_path(&slug).exists());
        assert!(
            prompt_dir.is_dir(),
            "the blocking prompt dir is left in place"
        );
    });
}

#[test]
fn load_routine_from_dir_applies_defaults_for_absent_optional_fields() {
    // A minimal routine.toml that omits prompt, enabled, timestamps, and id exercises the
    // default-fallback arms in load_routine_from_dir: prompt -> "", enabled -> true,
    // created_at/updated_at -> 0, and id -> dir_name (legacy fallback).
    with_override_home(|_home| {
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
    });
}

#[test]
fn load_routine_from_dir_missing_returns_none() {
    with_override_home(|_home| {
        assert!(load_routine_from_dir("rs-does-not-exist-zzz").is_none());
    });
}

#[test]
fn last_manual_trigger_at_persists_to_sidecar_not_routine_toml() {
    // Runtime trigger state is written to the gitignored `state.local.toml` sidecar and kept out
    // of the version-controlled `routine.toml`, then read back from the sidecar on load.
    with_override_home(|_home| {
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
    });
}

#[test]
fn write_routine_clears_stale_sidecar_when_untriggered() {
    // Re-writing a routine whose trigger state has been cleared removes the now-stale sidecar, so
    // the on-disk state mirrors the in-memory `None`.
    with_override_home(|_home| {
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

#[test]
fn load_routine_falls_back_to_legacy_last_triggered_in_routine_toml() {
    // A routine written by an older daemon stored `last_triggered_at` inside `routine.toml` and
    // has no sidecar. Load still surfaces the timestamp via the legacy-field fallback.
    with_override_home(|_home| {
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
    });
}

#[test]
fn load_routine_ignores_unparsable_sidecar() {
    // A malformed `state.local.toml` parses to `None` (rather than crashing the load), and with no
    // legacy field in `routine.toml` the routine loads with no trigger timestamp.
    with_override_home(|_home| {
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
    });
}

#[test]
fn load_routine_reads_scheduled_trigger_from_sidecar() {
    // `last_scheduled_trigger_at` lives in its own gitignored `scheduled.local.toml` sidecar,
    // written by the routine's `run.sh` at cron fire time, and is read back on load — independently
    // of the manual-trigger sidecar.
    with_override_home(|_home| {
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
    });
}

#[test]
fn load_routine_ignores_unparsable_scheduled_sidecar() {
    // A malformed `scheduled.local.toml` parses to `None` rather than crashing the load.
    with_override_home(|_home| {
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
    });
}

#[test]
fn write_routine_preserves_scheduler_written_scheduled_sidecar() {
    // The daemon never writes the scheduled sidecar, so re-persisting a routine (e.g. on startup or
    // an update) must leave the scheduler-stamped `scheduled.local.toml` untouched — the bug this
    // separate-file design exists to prevent.
    with_override_home(|_home| {
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
    });
}

#[test]
fn torn_routine_toml_loads_as_none() {
    // A truncated/garbage routine.toml (e.g. left by a crash mid-write) must not panic or load a
    // half-baked routine; the loader returns None and the routine is simply absent.
    with_override_home(|_home| {
        let slug = "rs-torn-toml-routine";
        let dir = crate::paths::routine_dir(slug);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(crate::paths::routine_toml_path(slug), "id = \"x\"\nschedu").unwrap();
        assert!(load_routine_from_dir(slug).is_none());
    });
}

#[test]
fn write_routine_leaves_no_tmp_residue() {
    with_override_home(|_home| {
        let id = "rs-no-residue-id";
        let title = "Rs No Residue Routine";
        let slug = slugify(title);
        write_routine(&make_routine(id, title)).unwrap();
        let residue = std::fs::read_dir(crate::paths::routine_dir(&slug))
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| entry.file_name().to_string_lossy().contains(".tmp"))
            .count();
        assert_eq!(residue, 0, "atomic_write must leave no .tmp files behind");
    });
}

#[test]
fn load_store_includes_written_routine() {
    with_override_home(|_home| {
        let id = "rs-loadstore-id";
        let title = "Rs Loadstore Routine";
        write_routine(&make_routine(id, title)).unwrap();
        let store = load_store();
        assert!(store.lock().unwrap().contains_key(id));
    });
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
    with_override_home(|_home| {
        remove_routine_dir("rs-never-created-zzz").unwrap();
    });
}

#[test]
fn migrate_routine_dirs_moves_legacy_uuid_dir_to_slug() {
    with_override_home(|_home| {
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

        // Legacy dir removed; canonical slug dir now holds toml + prompt sidecars, with the
        // legacy toml `prompt` field carried over into the new prompts/prompt.pure.md sidecar.
        assert!(!legacy_dir.exists(), "legacy UUID dir should be removed");
        assert!(crate::paths::routine_toml_path(&slug).exists());
        assert!(crate::paths::routine_compiled_prompt_path(&slug).exists());
        let loaded = load_routine_from_dir(&slug).unwrap();
        assert_eq!(loaded.id, id, "UUID id preserved across the dir migration");
        assert_eq!(loaded.prompt, "task");
    });
}

#[test]
fn repersist_routines_recreates_missing_prompt_sidecar() {
    with_override_home(|_home| {
        let id = "rs-repersist-id";
        let title = "Rs Repersist Routine";
        let slug = slugify(title);
        write_routine(&make_routine(id, title)).unwrap();
        // Simulate the sync-only state: prompt.compiled.md gone, only run.sh-style dir remains.
        std::fs::remove_file(crate::paths::routine_compiled_prompt_path(&slug)).unwrap();
        assert!(!crate::paths::routine_compiled_prompt_path(&slug).exists());

        let mut map = HashMap::new();
        map.insert(id.to_string(), make_routine(id, title));
        let store = Arc::new(Mutex::new(map));
        repersist_routines(&store);

        assert!(
            crate::paths::routine_compiled_prompt_path(&slug).exists(),
            "repersist should recreate the prompt sidecar"
        );
    });
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
    // A routine dir with no prompts/ subfolder and no legacy `prompt` field in routine.toml (nor
    // any routine.toml at all) still gets an (empty) prompt.pure.md written.
    let dir = scratch_dir("prompts-subfolder-no-legacy");
    std::fs::create_dir_all(&dir).unwrap();

    let routine = dir.join("no-legacy-prompt");
    std::fs::create_dir_all(&routine).unwrap();

    migrate_prompts_to_subfolder_from_dir(&dir);

    assert_eq!(
        std::fs::read_to_string(routine.join("prompts").join("prompt.pure.md")).unwrap(),
        ""
    );

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

// ─── New tests for previously uncovered lines ────────────────────────────────

#[test]
fn load_routine_from_dir_missing_title_returns_none() {
    // Covers L118: `let title = toml.title?;` — a TOML that has schedule and agent
    // but no `title` field causes `load_routine_from_dir` to return `None`.
    with_override_home(|_home| {
        let slug = "rs-no-title-zzz";
        let dir = crate::paths::routine_dir(slug);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            crate::paths::routine_toml_path(slug),
            "schedule = \"@daily\"\nagent = \"claude\"\n",
        )
        .unwrap();
        assert!(load_routine_from_dir(slug).is_none());
    });
}

#[test]
fn load_routine_from_dir_missing_schedule_returns_none() {
    // Covers L124: `schedule: toml.schedule?,` — a TOML with `title` and `agent` but
    // no `schedule` field causes `load_routine_from_dir` to return `None`.
    with_override_home(|_home| {
        let slug = "rs-no-schedule-zzz";
        let dir = crate::paths::routine_dir(slug);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            crate::paths::routine_toml_path(slug),
            "title = \"Rs No Schedule\"\nagent = \"claude\"\n",
        )
        .unwrap();
        assert!(load_routine_from_dir(slug).is_none());
    });
}

#[test]
fn load_routine_from_dir_missing_agent_returns_none() {
    // Covers L126: `agent: toml.agent?,` — a TOML with `title` and `schedule` but no
    // `agent` field causes `load_routine_from_dir` to return `None`.
    with_override_home(|_home| {
        let slug = "rs-no-agent-zzz";
        let dir = crate::paths::routine_dir(slug);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            crate::paths::routine_toml_path(slug),
            "title = \"Rs No Agent\"\nschedule = \"@daily\"\n",
        )
        .unwrap();
        assert!(load_routine_from_dir(slug).is_none());
    });
}

#[cfg(unix)]
#[test]
fn write_routine_fails_on_gitignore_write_error() {
    use std::os::unix::fs::PermissionsExt as _;
    // Covers L155: `std::fs::write(&gitignore, ..)? ` — the dir exists but is read-only
    // (and `.gitignore` is absent), so writing it fails and the error is propagated.
    with_override_home(|_home| {
        let title = "Rs Gitignore Fail Routine";
        let slug = slugify(title);
        let dir = crate::paths::routine_dir(&slug);
        // Create dir without a .gitignore, then lock it.
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o555)).unwrap();

        let result = write_routine(&make_routine("rs-gitignore-fail-id", title));

        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert!(
            result.is_err(),
            "write_routine should fail when .gitignore cannot be written"
        );
    });
}

#[cfg(unix)]
#[test]
fn write_routine_fails_on_routine_toml_write_error() {
    use std::os::unix::fs::PermissionsExt as _;
    // Covers L185: `atomic_write(&routine_toml_path(&slug), ..)? ` — `.gitignore` exists
    // (so that step is skipped), but the dir is read-only so the atomic write for
    // `routine.toml` (which creates a sibling temp file) fails.
    with_override_home(|_home| {
        let title = "Rs Toml Write Fail Routine";
        let slug = slugify(title);
        let dir = crate::paths::routine_dir(&slug);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            crate::paths::routine_gitignore_path(&slug),
            "*.local.*\n*.log\nrun.sh\n",
        )
        .unwrap();
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o555)).unwrap();

        let result = write_routine(&make_routine("rs-toml-write-fail-id", title));

        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert!(
            result.is_err(),
            "write_routine should fail when routine.toml cannot be written"
        );
    });
}

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

#[test]
fn snooze_fields_round_trip_through_sidecar_not_routine_toml() {
    // Snooze state is ephemeral/daemon-owned, like last_manual_trigger_at: it lives in the
    // gitignored state.local.toml sidecar, not the tracked routine.toml, and round-trips on load.
    with_override_home(|_home| {
        let title = "Rs Snooze Sidecar Routine";
        let slug = slugify(title);
        let mut routine = make_routine("rs-snooze-sidecar-id", title);
        routine.snoozed_until = Some(999_999);
        write_routine(&routine).unwrap();

        let toml_text = std::fs::read_to_string(crate::paths::routine_toml_path(&slug)).unwrap();
        assert!(
            !toml_text.contains("snoozed_until"),
            "routine.toml must not carry snooze state: {toml_text}"
        );
        let state_text = std::fs::read_to_string(crate::paths::routine_state_path(&slug)).unwrap();
        assert!(state_text.contains("snoozed_until"));

        let loaded = load_routine_from_dir(&slug).unwrap();
        assert_eq!(loaded.snoozed_until, Some(999_999));
        assert_eq!(loaded.skip_runs, None);
    });
}

#[test]
fn skip_runs_round_trips_and_clearing_both_removes_sidecar() {
    with_override_home(|_home| {
        let title = "Rs Skip Runs Sidecar Routine";
        let slug = slugify(title);
        let mut routine = make_routine("rs-skip-runs-sidecar-id", title);
        routine.skip_runs = Some(3);
        write_routine(&routine).unwrap();
        assert!(crate::paths::routine_state_path(&slug).exists());
        assert_eq!(load_routine_from_dir(&slug).unwrap().skip_runs, Some(3));

        routine.skip_runs = None;
        write_routine(&routine).unwrap();
        assert!(
            !crate::paths::routine_state_path(&slug).exists(),
            "sidecar should be removed once no runtime state (trigger or snooze) remains"
        );
        assert_eq!(load_routine_from_dir(&slug).unwrap().skip_runs, None);
    });
}
