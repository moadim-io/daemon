#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

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
fn invalid_data_error_maps_to_invalid_data_kind() {
    let err =
        invalid_data_error(serde_json::from_str::<serde_json::Value>("not json").unwrap_err());
    assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
}

#[test]
fn serialize_document_can_fail_for_coverage() {
    let previous = std::env::var_os("MOADIM_TEST_FORCE_CLAUDE_JSON_SERIALIZE_ERROR");
    // SAFETY: test harness only; restored below before returning.
    unsafe {
        std::env::set_var("MOADIM_TEST_FORCE_CLAUDE_JSON_SERIALIZE_ERROR", "1");
    }

    let err = serialize_document(&serde_json::json!({"projects": {}})).unwrap_err();

    // SAFETY: restore the prior value for subsequent tests.
    unsafe {
        match previous {
            Some(value) => {
                std::env::set_var("MOADIM_TEST_FORCE_CLAUDE_JSON_SERIALIZE_ERROR", value);
            }
            None => std::env::remove_var("MOADIM_TEST_FORCE_CLAUDE_JSON_SERIALIZE_ERROR"),
        }
    }

    assert_eq!(err.kind(), std::io::ErrorKind::Other);
}

#[cfg(unix)]
#[test]
fn prune_project_at_rejects_a_forced_lock_failure() {
    let previous = std::env::var_os("MOADIM_TEST_FORCE_CLAUDE_JSON_LOCK_ERROR");
    // SAFETY: test harness only; restored below before returning.
    unsafe {
        std::env::set_var("MOADIM_TEST_FORCE_CLAUDE_JSON_LOCK_ERROR", "1");
    }

    let dir = temp_dir("forced-lock-failure");
    let claude_json = dir.join(".claude.json");
    fs::write(&claude_json, r#"{"projects":{}}"#).unwrap();

    let result = prune_project_at(Some(claude_json), Path::new("/workbenches/gone"));

    // SAFETY: restore the prior value for subsequent tests.
    unsafe {
        match previous {
            Some(value) => std::env::set_var("MOADIM_TEST_FORCE_CLAUDE_JSON_LOCK_ERROR", value),
            None => std::env::remove_var("MOADIM_TEST_FORCE_CLAUDE_JSON_LOCK_ERROR"),
        }
    }

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

#[cfg(unix)]
#[test]
fn prune_project_at_errors_when_lock_file_cannot_be_created() {
    use std::os::unix::fs::PermissionsExt as _;

    // No `.claude.json.lock` exists yet, and the containing directory is read-only, so
    // `File::create(&lock_path)` in `prune_project_at` fails with a permission error —
    // exercising that `?` without touching `prune_locked` at all.
    let dir = temp_dir("lock-create-denied");
    let claude_json = dir.join(".claude.json");
    fs::write(&claude_json, r#"{"projects":{}}"#).unwrap();
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o555)).unwrap();

    let result = prune_project_at(Some(claude_json), Path::new("/workbenches/gone"));

    fs::set_permissions(&dir, fs::Permissions::from_mode(0o755)).unwrap();
    assert!(result.is_err());
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn prune_project_at_errors_when_claude_json_is_a_directory() {
    // `claude_json.exists()` is true for a directory too, so this reaches `prune_locked`, where
    // `fs::read_to_string` fails because the path isn't a regular file.
    let dir = temp_dir("claude-json-is-dir");
    let claude_json = dir.join(".claude.json");
    fs::create_dir(&claude_json).unwrap();

    let result = prune_project_at(Some(claude_json), Path::new("/workbenches/gone"));

    assert!(result.is_err());
    fs::remove_dir_all(&dir).unwrap();
}

#[cfg(unix)]
#[test]
fn prune_project_at_errors_when_atomic_write_cannot_create_its_temp_file() {
    use std::os::unix::fs::PermissionsExt as _;

    // The lock file already exists, so `File::create` on it only needs to truncate an existing
    // directory entry (no directory-write permission required). The matching `projects` entry
    // still gets removed in memory, but the read-only directory then rejects the sibling temp
    // file `atomic_write` creates before its rename, exercising that `?` specifically.
    let dir = temp_dir("atomic-write-denied");
    let claude_json = dir.join(".claude.json");
    let workbench = "/home/u/.moadim/workbenches/my-routine-1700000000";
    fs::write(
        &claude_json,
        format!(r#"{{"projects":{{"{workbench}":{{}}}}}}"#),
    )
    .unwrap();
    fs::write(lock_path_for(&claude_json), "").unwrap();
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o555)).unwrap();

    let result = prune_project_at(Some(claude_json), Path::new(workbench));

    fs::set_permissions(&dir, fs::Permissions::from_mode(0o755)).unwrap();
    assert!(result.is_err());
    fs::remove_dir_all(&dir).unwrap();
}

#[cfg(unix)]
#[test]
fn lock_exclusive_and_unlock_error_on_a_closed_fd() {
    use std::os::fd::AsRawFd as _;

    // `flock(2)` on a closed descriptor fails with EBADF, exercising the `Err` arm of both
    // `lock_exclusive` and `unlock` without depending on any real filesystem lock contention.
    let dir = temp_dir("flock-closed-fd");
    let path = dir.join("lockfile");
    let file = File::create(&path).unwrap();
    // SAFETY: `file` is not used again except via the (now-invalid) fd captured by `lock_exclusive`
    // and `unlock` below; this deliberately makes `flock` observe a closed descriptor.
    unsafe {
        libc::close(file.as_raw_fd());
    }

    assert!(lock_exclusive(&file).is_err());
    assert!(unlock(&file).is_err());

    #[allow(
        clippy::mem_forget,
        reason = "the fd was already manually closed above; letting `file`'s Drop run would close \
                  it a second time (or, worse, a reused fd from an unrelated file), so forgetting \
                  it is deliberate here, not a leak"
    )]
    std::mem::forget(file);
    fs::remove_dir_all(&dir).unwrap();
}
