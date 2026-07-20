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
    let home = scratch_dir("override-home-gitignore");
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
        env: std::collections::HashMap::new(),
    }
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
