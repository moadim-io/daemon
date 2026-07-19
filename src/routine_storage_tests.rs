#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use super::*;
use crate::routines::{slugify, Repository, Routine};

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
        assert!(crate::paths::routine_cron_path(&slug).exists());
        assert!(crate::paths::routine_pure_prompt_path(&slug).exists());
        assert!(crate::paths::routine_compiled_prompt_path(&slug).exists());
        assert!(crate::paths::routine_gitignore_path(&slug).exists());
        let toml_text = std::fs::read_to_string(crate::paths::routine_toml_path(&slug)).unwrap();
        assert!(
            !toml_text.contains("schedule"),
            "routine.toml must not carry the schedule: {toml_text}"
        );
        assert!(
            !toml_text.contains("prompt"),
            "routine.toml must not carry the prompt: {toml_text}"
        );
        assert_eq!(
            std::fs::read_to_string(crate::paths::routine_cron_path(&slug)).unwrap(),
            "@daily\n"
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
fn load_routine_from_dir_applies_defaults_for_absent_optional_fields() {
    // A minimal current-layout routine (schedule.cron + routine.toml) that omits prompt, enabled,
    // timestamps, and id exercises the default-fallback arms in load_routine_from_dir:
    // prompt -> "", enabled -> true, created_at/updated_at -> 0, and id -> dir_name (legacy fallback).
    with_override_home(|_home| {
        let slug = "rs-defaults-routine";
        let dir = crate::paths::routine_dir(slug);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            crate::paths::routine_toml_path(slug),
            "title = \"Rs Defaults Routine\"\nagent = \"claude\"\n",
        )
        .unwrap();
        std::fs::write(crate::paths::routine_cron_path(slug), "@daily\n").unwrap();

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
fn load_routine_falls_back_to_legacy_schedule_in_routine_toml() {
    // Older routine dirs kept the schedule in routine.toml; the loader still accepts that until
    // the next repersist writes schedule.cron.
    with_override_home(|_home| {
        let slug = "rs-legacy-schedule-routine";
        let dir = crate::paths::routine_dir(slug);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            crate::paths::routine_toml_path(slug),
            "schedule = \"@hourly\"\ntitle = \"Rs Legacy Schedule\"\nagent = \"claude\"\n",
        )
        .unwrap();

        let loaded = load_routine_from_dir(slug).unwrap();
        assert_eq!(loaded.schedule, "@hourly");
        assert!(!crate::paths::routine_cron_path(slug).exists());
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
fn load_routine_reads_scheduled_trigger_from_log() {
    // `last_scheduled_trigger_at` is read from the last line of `scheduled.log`, written by the
    // cron shell command at each fire, independently of the manual-trigger log.
    with_override_home(|_home| {
        let title = "Rs Scheduled Sidecar Routine";
        let slug = slugify(title);
        write_routine(&make_routine("rs-scheduled-id", title)).unwrap();
        // Simulate two cron fires appended to scheduled.log.
        std::fs::write(
            crate::paths::routine_scheduled_log_path(&slug),
            "1000\n4242\n",
        )
        .unwrap();

        let loaded = load_routine_from_dir(&slug).unwrap();
        // The last line (4242) wins.
        assert_eq!(loaded.last_scheduled_trigger_at, Some(4242));
        // The scheduled timestamp is distinct from the (unset) manual one.
        assert_eq!(loaded.last_manual_trigger_at, None);
    });
}

#[test]
fn load_routine_ignores_unparsable_scheduled_log() {
    // A `scheduled.log` with no parsable timestamp lines yields `None` rather than crashing.
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
            crate::paths::routine_scheduled_log_path(slug),
            "not a timestamp\n",
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
fn write_routine_preserves_scheduler_written_scheduled_log() {
    // The daemon never writes `scheduled.log`, so re-persisting a routine must leave the
    // cron-appended log untouched — the same invariant that motivated the separate-file design.
    with_override_home(|_home| {
        let title = "Rs Preserve Scheduled Routine";
        let slug = slugify(title);
        let mut routine = make_routine("rs-preserve-scheduled-id", title);
        write_routine(&routine).unwrap();

        // Simulate a scheduled cron firing appending to scheduled.log.
        std::fs::write(crate::paths::routine_scheduled_log_path(&slug), "55\n").unwrap();

        // A subsequent daemon-side write (manual trigger recorded, routine updated, repersist, …).
        routine.last_manual_trigger_at = Some(7);
        write_routine(&routine).unwrap();
        crate::routine_storage::append_manual_trigger_log(&slug, 7);

        assert!(
            crate::paths::routine_scheduled_log_path(&slug).exists(),
            "daemon write must not remove the scheduler-owned log"
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
        assert!(crate::paths::routine_cron_path(&slug).exists());
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
        // Simulate the sync-only state: prompt.compiled.local.md and schedule.cron are gone.
        std::fs::remove_file(crate::paths::routine_compiled_prompt_path(&slug)).unwrap();
        std::fs::remove_file(crate::paths::routine_cron_path(&slug)).unwrap();
        assert!(!crate::paths::routine_compiled_prompt_path(&slug).exists());
        assert!(!crate::paths::routine_cron_path(&slug).exists());

        let mut map = HashMap::new();
        map.insert(id.to_string(), make_routine(id, title));
        let store = Arc::new(Mutex::new(map));
        repersist_routines(&store);

        assert!(
            crate::paths::routine_compiled_prompt_path(&slug).exists(),
            "repersist should recreate the prompt sidecar"
        );
        assert!(
            crate::paths::routine_cron_path(&slug).exists(),
            "repersist should recreate the cron sidecar"
        );
    });
}

#[test]
fn write_routine_seeds_gitignore_with_all_required_patterns() {
    with_override_home(|_home| {
        let id = "rs-gitignore-seed-id";
        let title = "Rs Gitignore Seed Routine";
        let slug = slugify(title);
        write_routine(&make_routine(id, title)).unwrap();

        let content = std::fs::read_to_string(crate::paths::routine_gitignore_path(&slug)).unwrap();
        for pattern in ["*.local.*", "*.log", "run.sh"] {
            assert!(
                content.lines().any(|line| line == pattern),
                "missing required pattern {pattern:?} in {content:?}"
            );
        }

        // Writing again with the gitignore already fully seeded exercises the no-op / early-return
        // branch of `ensure_routine_gitignore` and must leave the file byte-for-byte unchanged.
        write_routine(&make_routine(id, title)).unwrap();
        let content_again =
            std::fs::read_to_string(crate::paths::routine_gitignore_path(&slug)).unwrap();
        assert_eq!(
            content, content_again,
            "an already-satisfied gitignore must be left untouched"
        );
    });
}

#[test]
fn write_routine_heals_a_legacy_gitignore_missing_required_patterns() {
    with_override_home(|_home| {
        let id = "rs-gitignore-heal-id";
        let title = "Rs Gitignore Heal Routine";
        let slug = slugify(title);
        std::fs::create_dir_all(crate::paths::routine_dir(&slug)).unwrap();
        // Simulate an install from before `run.sh` was added to the required patterns, plus a
        // user-added custom entry that reconciliation must preserve. No trailing newline,
        // exercising the "append one before the new patterns" branch too.
        std::fs::write(
            crate::paths::routine_gitignore_path(&slug),
            "*.local.*\n*.log\nmy-custom-pattern",
        )
        .unwrap();

        write_routine(&make_routine(id, title)).unwrap();

        let content = std::fs::read_to_string(crate::paths::routine_gitignore_path(&slug)).unwrap();
        assert!(content.lines().any(|line| line == "run.sh"));
        assert!(
            content.lines().any(|line| line == "my-custom-pattern"),
            "user-added pattern must survive reconciliation: {content:?}"
        );
    });
}
