#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and cases do not need doc comments"
)]

use std::path::{Path, PathBuf};

use super::super::bundle::Bundle;
use super::run_export;

fn scratch_dir(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!("moadim-export-{tag}-{}", uuid::Uuid::new_v4()))
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

fn write(root: &Path, rel: &str, content: &[u8]) {
    let path = root.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, content).unwrap();
}

fn seed_config_tree(home: &Path) -> PathBuf {
    let root = home.join(".config").join("moadim");
    write(&root, "notifications.toml", b"[[on_failure_webhook]]\n");
    write(&root, "machine.local.toml", b"name = 'secret-host'\n");
    write(&root, "daemon.log", b"log line\n");
    write(&root, "agents/claude.toml", b"command = 'claude'\n");
    write(&root, "agents/README.md", b"registry docs\n");
    write(&root, "routines/daily/routine.toml", b"title = 'Daily'\n");
    write(&root, "routines/daily/schedule.cron", b"@daily\n");
    write(&root, "routines/daily/state.local.toml", b"snoozed = 1\n");
    write(&root, "routines/daily/prompts/prompt.pure.md", b"do work\n");
    write(
        &root,
        "routines/daily/prompts/prompt.compiled.local.md",
        b"compiled\n",
    );
    root
}

#[test]
fn export_to_file_includes_tracked_and_excludes_local_files() {
    with_override_home(|home| {
        seed_config_tree(home);
        let out = home.join("bundle.json");
        assert_eq!(run_export(Some(out.clone())), 0);
        let bundle: Bundle = serde_json::from_str(&std::fs::read_to_string(&out).unwrap()).unwrap();
        assert_eq!(bundle.version, 1);
        let keys: Vec<&str> = bundle.files.keys().map(String::as_str).collect();
        assert_eq!(
            keys,
            vec![
                "agents/claude.toml",
                "notifications.toml",
                "routines/daily/prompts/prompt.pure.md",
                "routines/daily/routine.toml",
                "routines/daily/schedule.cron",
            ]
        );
        assert_eq!(
            bundle.files.get("routines/daily/routine.toml").unwrap(),
            "title = 'Daily'\n"
        );
    });
}

#[test]
fn export_to_stdout_succeeds() {
    with_override_home(|home| {
        seed_config_tree(home);
        assert_eq!(run_export(None), 0);
    });
}

#[test]
fn export_skips_non_utf8_tracked_files() {
    with_override_home(|home| {
        let root = seed_config_tree(home);
        write(&root, "routines/bad/routine.toml", &[0xff, 0xfe, 0x00]);
        let out = home.join("bundle.json");
        assert_eq!(run_export(Some(out.clone())), 0);
        let bundle: Bundle = serde_json::from_str(&std::fs::read_to_string(&out).unwrap()).unwrap();
        assert!(!bundle.files.contains_key("routines/bad/routine.toml"));
        assert!(bundle.files.contains_key("routines/daily/routine.toml"));
    });
}

#[test]
fn export_subcommand_dispatches_through_the_data_cli() {
    with_override_home(|home| {
        seed_config_tree(home);
        let out = home.join("bundle.json");
        let args = vec!["export".to_string(), "--out".to_string(), display(&out)];
        assert_eq!(crate::commands::run(args), 0);
        assert!(out.exists());
    });
}

fn display(path: &Path) -> String {
    path.display().to_string()
}

#[test]
fn export_fails_when_config_dir_is_missing() {
    with_override_home(|_home| {
        assert_eq!(run_export(None), 1);
    });
}

#[test]
fn export_fails_when_out_file_is_unwritable() {
    with_override_home(|home| {
        seed_config_tree(home);
        let out = home.join("missing-dir").join("bundle.json");
        assert_eq!(run_export(Some(out)), 1);
    });
}
