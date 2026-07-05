#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use super::*;

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
