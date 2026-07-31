#![allow(
    clippy::wildcard_imports,
    clippy::too_many_arguments,
    clippy::needless_pass_by_value,
    dead_code,
    reason = "split-out module preserves existing code while keeping files under the linecheck limit"
)]
use super::*;

#[test]
fn append_persisted_run_rotates_an_oversized_log_before_appending() {
    let _home = TempHome::set();
    let path = crate::paths::routine_run_history_path("big-id");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, vec![b'z'; (RUN_HISTORY_MAX_BYTES + 1) as usize]).unwrap();

    append_persisted_run("big-id", &sample_run("my-routine-1000", 1000));

    assert!(
        path.with_extension("log.1").exists(),
        "the oversized log must be rotated aside"
    );
    assert_eq!(
        read_persisted_runs("big-id"),
        vec![sample_run("my-routine-1000", 1000)],
        "the fresh log must contain only the newly appended run, not the rotated-away content"
    );
}

#[test]
fn read_persisted_runs_preserves_history_across_rotation() {
    let _home = TempHome::set();
    let path = crate::paths::routine_run_history_path("rotate-id");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();

    // Seed an oversized `runs.log` containing one real, pre-rotation run.
    let old_run = sample_run("my-routine-1000", 1000);
    let mut content = serde_json::to_string(&old_run).unwrap();
    content.push('\n');
    content.push_str(&"x".repeat(RUN_HISTORY_MAX_BYTES as usize));
    std::fs::write(&path, content).unwrap();

    // Appending a new run rotates the oversized log out of the way first.
    let new_run = sample_run("my-routine-2000", 2000);
    append_persisted_run("rotate-id", &new_run);

    let runs = read_persisted_runs("rotate-id");
    assert!(
        runs.contains(&old_run),
        "the pre-rotation run must still be readable after rotation (#1277)"
    );
    assert!(
        runs.contains(&new_run),
        "the newly appended run must also be readable"
    );
}

#[cfg(unix)]
#[test]
fn append_persisted_run_creates_owner_only_log_and_dir() {
    use std::os::unix::fs::PermissionsExt;

    let _home = TempHome::set();
    append_persisted_run("perm-id", &sample_run("my-routine-1000", 1000));

    let path = crate::paths::routine_run_history_path("perm-id");
    let file_mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
    assert_eq!(
        file_mode, 0o600,
        "runs.log should be 0600, got {file_mode:o}"
    );

    let dir_mode = std::fs::metadata(path.parent().unwrap())
        .unwrap()
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(
        dir_mode, 0o700,
        "routine dir should be 0700, got {dir_mode:o}"
    );
}
