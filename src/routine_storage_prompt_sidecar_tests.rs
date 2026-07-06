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
