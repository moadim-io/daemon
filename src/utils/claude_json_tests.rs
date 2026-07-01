#![allow(clippy::missing_docs_in_private_items)]

use super::*;

/// Create a fresh, unique tempdir for a test to write its own `claude.json` fixture into.
fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "moadim-claude-json-test-{name}-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn prune_project_at_returns_false_when_home_is_unresolvable() {
    let removed = prune_project_at(None, Path::new("/workbenches/anything-123"));
    assert!(!removed.unwrap());
}

#[test]
fn prune_project_at_returns_false_when_claude_json_is_missing() {
    let dir = temp_dir("missing-file");
    let claude_json = dir.join(".claude.json");

    let removed = prune_project_at(Some(claude_json), Path::new("/workbenches/anything-123"));

    assert!(!removed.unwrap());
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn prune_project_at_removes_the_matching_entry_and_rewrites_the_file() {
    let dir = temp_dir("removes-match");
    let claude_json = dir.join(".claude.json");
    let workbench = "/home/u/.moadim/workbenches/my-routine-1700000000";
    fs::write(
        &claude_json,
        format!(
            r#"{{"projects":{{"{workbench}":{{"hasTrustDialogAccepted":true}},"/other/wb":{{}}}}}}"#
        ),
    )
    .unwrap();

    let removed = prune_project_at(Some(claude_json.clone()), Path::new(workbench));

    assert!(removed.unwrap());
    let rewritten: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&claude_json).unwrap()).unwrap();
    let projects = rewritten.get("projects").unwrap().as_object().unwrap();
    assert!(!projects.contains_key(workbench), "entry must be pruned");
    assert!(
        projects.contains_key("/other/wb"),
        "unrelated entries must survive"
    );

    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn prune_project_at_leaves_file_untouched_when_no_matching_entry() {
    let dir = temp_dir("no-match");
    let claude_json = dir.join(".claude.json");
    let original = r#"{"projects":{"/other/wb":{}}}"#;
    fs::write(&claude_json, original).unwrap();

    let removed = prune_project_at(Some(claude_json.clone()), Path::new("/workbenches/gone"));

    assert!(!removed.unwrap());
    assert_eq!(fs::read_to_string(&claude_json).unwrap(), original);

    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn prune_project_at_returns_false_when_projects_key_is_absent() {
    let dir = temp_dir("no-projects-key");
    let claude_json = dir.join(".claude.json");
    fs::write(&claude_json, r#"{"other":true}"#).unwrap();

    let removed = prune_project_at(Some(claude_json), Path::new("/workbenches/gone"));

    assert!(!removed.unwrap());
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn prune_project_at_errors_on_malformed_json() {
    let dir = temp_dir("malformed");
    let claude_json = dir.join(".claude.json");
    fs::write(&claude_json, "not json").unwrap();

    let result = prune_project_at(Some(claude_json), Path::new("/workbenches/gone"));

    assert!(result.is_err());
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn prune_project_delegates_to_the_resolved_claude_json_path() {
    // End-to-end through the public entry point: point `MOADIM_HOME_OVERRIDE` at a fresh home with
    // no `~/.claude.json`, so this only exercises the `claude_json_path()` resolution plus the
    // "file missing" short-circuit, without touching the caller's real home.
    let dir = temp_dir("public-entrypoint");
    let previous = std::env::var_os("MOADIM_HOME_OVERRIDE");
    // SAFETY: tests in this crate run single-threaded (RUST_TEST_THREADS=1); restored below.
    unsafe {
        std::env::set_var("MOADIM_HOME_OVERRIDE", &dir);
    }

    let removed = prune_project(Path::new("/workbenches/anything"));

    // SAFETY: single-threaded harness; restore the saved override.
    unsafe {
        match previous {
            Some(value) => std::env::set_var("MOADIM_HOME_OVERRIDE", value),
            None => std::env::remove_var("MOADIM_HOME_OVERRIDE"),
        }
    }

    assert!(!removed.unwrap());
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn lock_path_for_appends_dot_lock() {
    let path = lock_path_for(Path::new("/home/u/.claude.json"));
    assert_eq!(path, PathBuf::from("/home/u/.claude.json.lock"));
}

#[test]
fn lock_exclusive_and_unlock_round_trip_on_a_real_file() {
    let dir = temp_dir("flock-roundtrip");
    let path = dir.join("lockfile");
    let file = File::create(&path).unwrap();

    lock_exclusive(&file).unwrap();
    unlock(&file).unwrap();

    fs::remove_dir_all(&dir).unwrap();
}
