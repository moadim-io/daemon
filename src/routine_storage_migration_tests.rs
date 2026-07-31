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
        schedules: vec![],
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
        env: std::collections::HashMap::new(),
        auto_disabled_reason: None,
        consecutive_failures: 0,
        failure_threshold: None,
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
    // SAFETY: single-threaded test execution.
    unsafe {
        match previous {
            Some(value) => std::env::set_var("MOADIM_HOME_OVERRIDE", value),
            None => std::env::remove_var("MOADIM_HOME_OVERRIDE"),
        }
    }
    let _ = std::fs::remove_dir_all(&home);
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
include!("routine_storage_migration_tests_part3.rs");
