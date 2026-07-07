#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use super::*;

struct TempHome(std::path::PathBuf);

impl TempHome {
    fn set() -> Self {
        let dir = std::env::temp_dir().join(format!("moadim-runhisttest-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("create temp home");
        // SAFETY: single-threaded test execution.
        unsafe {
            std::env::set_var("MOADIM_HOME_OVERRIDE", &dir);
        }
        Self(dir)
    }
}

impl Drop for TempHome {
    fn drop(&mut self) {
        // SAFETY: single-threaded test execution.
        unsafe {
            std::env::remove_var("MOADIM_HOME_OVERRIDE");
        }
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn sample_run(workbench: &str, started_at: u64) -> PersistedRun {
    PersistedRun {
        workbench: workbench.to_string(),
        started_at,
        finished_at: started_at + 5,
        status: RunStatus::Success,
        exit_code: Some(0),
    }
}

#[test]
fn read_persisted_runs_empty_when_file_absent() {
    let _home = TempHome::set();
    assert_eq!(read_persisted_runs("no-such-id"), vec![]);
}

#[test]
fn append_then_read_round_trips() {
    let _home = TempHome::set();
    let run = sample_run("my-routine-1000", 1000);
    append_persisted_run("some-id", &run);
    assert_eq!(read_persisted_runs("some-id"), vec![run]);
}

#[test]
fn append_accumulates_multiple_lines() {
    let _home = TempHome::set();
    append_persisted_run("some-id", &sample_run("my-routine-1000", 1000));
    append_persisted_run("some-id", &sample_run("my-routine-2000", 2000));
    let runs = read_persisted_runs("some-id");
    assert_eq!(runs.len(), 2);
}

#[test]
fn read_persisted_runs_skips_malformed_lines() {
    let _home = TempHome::set();
    append_persisted_run("some-id", &sample_run("my-routine-1000", 1000));
    // Corrupt the file with a trailing malformed line, mimicking a crash mid-write.
    let path = crate::paths::routine_run_history_path("some-id");
    let mut existing = std::fs::read_to_string(&path).unwrap();
    existing.push_str("{not valid json\n");
    std::fs::write(&path, existing).unwrap();

    let runs = read_persisted_runs("some-id");
    assert_eq!(runs.len(), 1, "the malformed line is skipped, not fatal");
}

#[test]
fn read_exit_code_none_when_file_absent() {
    let dir = std::env::temp_dir().join(format!("moadim-exitcode-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    assert_eq!(read_exit_code(&dir), None);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn read_exit_code_parses_valid_content() {
    let dir = std::env::temp_dir().join(format!("moadim-exitcode-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("exit_code"), "0").unwrap();
    assert_eq!(read_exit_code(&dir), Some(0));
    std::fs::write(dir.join("exit_code"), "17").unwrap();
    assert_eq!(read_exit_code(&dir), Some(17));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn read_exit_code_none_when_content_unparseable() {
    let dir = std::env::temp_dir().join(format!("moadim-exitcode-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("exit_code"), "not-a-number").unwrap();
    assert_eq!(read_exit_code(&dir), None);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn append_persisted_run_creates_parent_dir_when_absent() {
    let _home = TempHome::set();
    assert!(!crate::paths::routine_run_history_path("fresh-id")
        .parent()
        .unwrap()
        .exists());
    append_persisted_run("fresh-id", &sample_run("my-routine-1000", 1000));
    assert_eq!(read_persisted_runs("fresh-id").len(), 1);
}

#[test]
fn append_persisted_run_is_best_effort_when_path_unwritable() {
    let _home = TempHome::set();
    // Place a regular file where the routine's directory should be, so `create_dir_all` fails.
    let routine_dir = crate::paths::routine_run_history_path("blocked-id")
        .parent()
        .unwrap()
        .to_path_buf();
    std::fs::create_dir_all(routine_dir.parent().unwrap()).unwrap();
    std::fs::write(&routine_dir, b"i am a file, not a dir").unwrap();

    // Must not panic; the failure is logged and swallowed.
    append_persisted_run("blocked-id", &sample_run("my-routine-1000", 1000));
    assert_eq!(read_persisted_runs("blocked-id"), vec![]);
}

#[test]
fn has_persisted_run_false_when_file_absent() {
    let _home = TempHome::set();
    assert!(!has_persisted_run("no-such-id", "my-routine-1000"));
}

#[test]
fn has_persisted_run_true_only_for_matching_workbench() {
    let _home = TempHome::set();
    append_persisted_run("some-id", &sample_run("my-routine-1000", 1000));

    assert!(has_persisted_run("some-id", "my-routine-1000"));
    assert!(!has_persisted_run("some-id", "my-routine-2000"));
    assert!(!has_persisted_run("other-id", "my-routine-1000"));
}

#[test]
fn rotate_run_history_if_oversized_is_a_no_op_when_file_is_missing() {
    let path = std::env::temp_dir().join(format!("moadim-runs-missing-{}", uuid::Uuid::new_v4()));
    rotate_run_history_if_oversized(&path);
    assert!(!path.exists());
}

#[test]
fn rotate_run_history_if_oversized_leaves_small_files_in_place() {
    let base = std::env::temp_dir().join(format!("moadim-runs-small-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&base).unwrap();
    let path = base.join("runs.log");
    std::fs::write(&path, b"a few bytes").unwrap();

    rotate_run_history_if_oversized(&path);

    assert!(path.exists(), "file under the cap must not be rotated");
    assert!(!path.with_extension("log.1").exists());
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn rotate_run_history_if_oversized_rolls_the_file_past_the_cap() {
    let base = std::env::temp_dir().join(format!("moadim-runs-big-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&base).unwrap();
    let path = base.join("runs.log");
    std::fs::write(&path, vec![b'x'; (RUN_HISTORY_MAX_BYTES + 1) as usize]).unwrap();

    rotate_run_history_if_oversized(&path);

    assert!(
        !path.exists(),
        "the oversized file must be moved out of the way"
    );
    assert!(
        path.with_extension("log.1").exists(),
        "the oversized file must land at the .1 sibling"
    );
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn rotate_run_history_if_oversized_replaces_a_previous_1_file() {
    let base = std::env::temp_dir().join(format!("moadim-runs-replace-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&base).unwrap();
    let path = base.join("runs.log");
    std::fs::write(&path, vec![b'y'; (RUN_HISTORY_MAX_BYTES + 1) as usize]).unwrap();
    let rotated = path.with_extension("log.1");
    std::fs::write(&rotated, b"stale rotated content").unwrap();

    rotate_run_history_if_oversized(&path);

    assert!(rotated.exists());
    assert_eq!(
        std::fs::metadata(&rotated).unwrap().len(),
        RUN_HISTORY_MAX_BYTES + 1,
        "rotation must replace a stale .1 file with the freshly-rolled one"
    );
    let _ = std::fs::remove_dir_all(&base);
}

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
