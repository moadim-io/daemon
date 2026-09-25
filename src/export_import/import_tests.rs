#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and cases do not need doc comments"
)]

use std::path::{Path, PathBuf};

use super::run_import;

fn scratch_dir(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!("moadim-import-{tag}-{}", uuid::Uuid::new_v4()))
}

/// Point `MOADIM_HOME_OVERRIDE` at a fresh tempdir home for the duration of `body`.
fn with_override_home(body: impl FnOnce(&Path)) {
    let home = scratch_dir("home");
    std::fs::create_dir_all(&home).unwrap();
    let previous = std::env::var_os("MOADIM_HOME_OVERRIDE");
    // SAFETY: tests in this crate run single-threaded per binary (RUST_TEST_THREADS=1).
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

fn config_root(home: &Path) -> PathBuf {
    home.join(".config").join("moadim")
}

/// Write a version-1 bundle holding `files` to a fresh path under `home` and return that path.
fn write_bundle(home: &Path, files: &[(&str, &str)]) -> PathBuf {
    let map: serde_json::Map<String, serde_json::Value> = files
        .iter()
        .map(|(rel, content)| ((*rel).to_string(), serde_json::Value::from(*content)))
        .collect();
    let bundle = serde_json::json!({ "version": 1, "files": map });
    let path = home.join(format!("bundle-{}.json", uuid::Uuid::new_v4()));
    std::fs::write(&path, serde_json::to_string(&bundle).unwrap()).unwrap();
    path
}

#[test]
fn import_creates_files_through_the_config_root() {
    with_override_home(|home| {
        let bundle = write_bundle(
            home,
            &[
                ("routines/daily/routine.toml", "title = 'Daily'\n"),
                ("routines/daily/schedule.cron", "@daily\n"),
                ("routines/daily/prompts/prompt.pure.md", "do work\n"),
                ("agents/claude.toml", "command = 'claude'\n"),
            ],
        );
        assert_eq!(run_import(&bundle, false, false), 0);
        let root = config_root(home);
        let toml = std::fs::read_to_string(root.join("routines/daily/routine.toml")).unwrap();
        assert_eq!(toml, "title = 'Daily'\n");
        let prompt =
            std::fs::read_to_string(root.join("routines/daily/prompts/prompt.pure.md")).unwrap();
        assert_eq!(prompt, "do work\n");
    });
}

#[test]
fn import_skips_existing_files_by_default_and_overwrites_with_force() {
    with_override_home(|home| {
        let root = config_root(home);
        let target = root.join("routines/daily/routine.toml");
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        std::fs::write(&target, "title = 'Original'\n").unwrap();
        let bundle = write_bundle(home, &[("routines/daily/routine.toml", "title = 'New'\n")]);
        assert_eq!(run_import(&bundle, false, false), 0);
        assert_eq!(
            std::fs::read_to_string(&target).unwrap(),
            "title = 'Original'\n"
        );
        assert_eq!(run_import(&bundle, false, true), 0);
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "title = 'New'\n");
    });
}

#[test]
fn dry_run_reports_the_plan_without_writing() {
    with_override_home(|home| {
        let root = config_root(home);
        let existing = root.join("agents").join("claude.toml");
        std::fs::create_dir_all(existing.parent().unwrap()).unwrap();
        std::fs::write(&existing, "command = 'claude'\n").unwrap();
        let bundle = write_bundle(
            home,
            &[
                ("agents/claude.toml", "command = 'changed'\n"),
                ("routines/daily/routine.toml", "title = 'Daily'\n"),
            ],
        );
        assert_eq!(run_import(&bundle, true, false), 0);
        assert_eq!(run_import(&bundle, true, true), 0);
        assert!(!root.join("routines/daily/routine.toml").exists());
        assert_eq!(
            std::fs::read_to_string(&existing).unwrap(),
            "command = 'claude'\n"
        );
    });
}

#[test]
fn import_subcommand_dispatches_through_the_data_cli() {
    with_override_home(|home| {
        let bundle = write_bundle(
            home,
            &[("routines/daily/routine.toml", "title = 'Daily'\n")],
        );
        let args = vec![
            "import".to_string(),
            bundle.display().to_string(),
            "--dry-run".to_string(),
        ];
        assert_eq!(crate::commands::run(args), 0);
        assert!(!config_root(home)
            .join("routines/daily/routine.toml")
            .exists());
    });
}

#[test]
fn import_rejects_a_missing_or_malformed_bundle_file() {
    with_override_home(|home| {
        assert_eq!(run_import(&home.join("absent.json"), false, false), 2);
        let not_json = home.join("not-json.txt");
        std::fs::write(&not_json, "not a bundle").unwrap();
        assert_eq!(run_import(&not_json, false, false), 2);
    });
}

#[test]
fn import_rejects_an_unsupported_bundle_version() {
    with_override_home(|home| {
        let path = home.join("v9.json");
        std::fs::write(&path, r#"{"version": 9, "files": {}}"#).unwrap();
        assert_eq!(run_import(&path, false, false), 2);
    });
}

#[test]
fn import_rejects_untracked_and_traversal_paths_before_writing() {
    with_override_home(|home| {
        let sneaky = write_bundle(
            home,
            &[
                ("routines/daily/routine.toml", "title = 'Daily'\n"),
                ("routines/daily/state.local.toml", "snoozed = 1\n"),
            ],
        );
        assert_eq!(run_import(&sneaky, false, false), 2);
        assert!(!config_root(home)
            .join("routines/daily/routine.toml")
            .exists());
        let traversal = write_bundle(home, &[("routines/../../../evil.toml", "boom = 1\n")]);
        assert_eq!(run_import(&traversal, false, false), 2);
    });
}

#[test]
fn import_rejects_a_routine_toml_that_is_not_valid_toml() {
    with_override_home(|home| {
        let bundle = write_bundle(home, &[("routines/daily/routine.toml", "not [valid toml")]);
        assert_eq!(run_import(&bundle, false, false), 2);
    });
}

#[test]
fn import_reports_a_write_failure() {
    with_override_home(|home| {
        let root = config_root(home);
        std::fs::create_dir_all(&root).unwrap();
        // A plain file where the `routines/` directory should go makes `create_dir_all` fail.
        std::fs::write(root.join("routines"), "in the way").unwrap();
        let bundle = write_bundle(
            home,
            &[("routines/daily/routine.toml", "title = 'Daily'\n")],
        );
        assert_eq!(run_import(&bundle, false, false), 1);
    });
}
