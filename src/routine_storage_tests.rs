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
    crate::test_fixtures::routine_fixture(id, title).build()
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
        assert!(
            !crate::paths::routine_gitignore_path(&slug).exists(),
            "per-routine .gitignore is no longer generated"
        );
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
include!("load_routine_from_dir_applies_defaults_for_absent_optional_fields_tests.rs");
