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
