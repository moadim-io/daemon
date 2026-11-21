use super::*;

use crate::routines::model::{KillReason, RunStatus};

fn status(exit_code: Option<i32>, kill: Option<KillReason>, alive: bool) -> RunStatus {
    derive_run_status(exit_code, kill, || alive)
}

#[test]
fn derive_run_status_success_for_zero_exit() {
    assert_eq!(status(Some(0), None, false), RunStatus::Success);
}

#[test]
fn derive_run_status_failed_for_nonzero_exit() {
    assert_eq!(status(Some(1), None, false), RunStatus::Failed);
    assert_eq!(status(Some(137), None, true), RunStatus::Failed);
}

#[test]
fn derive_run_status_killed_when_kill_recorded_with_no_exit_code() {
    assert_eq!(
        status(None, Some(KillReason::MaxRuntime), false),
        RunStatus::Killed
    );
    // The watchdog kills the session before recording, so liveness must not override the kill.
    assert_eq!(
        status(None, Some(KillReason::MaxRuntime), true),
        RunStatus::Killed
    );
}

#[test]
fn derive_run_status_running_when_session_alive_and_no_outcome() {
    assert_eq!(status(None, None, true), RunStatus::Running);
}

#[test]
fn derive_run_status_unknown_when_session_gone_and_no_outcome() {
    assert_eq!(status(None, None, false), RunStatus::Unknown);
}

#[test]
fn derive_run_status_skips_liveness_check_when_outcome_known() {
    let mut probed = false;
    let result = derive_run_status(Some(0), None, || {
        probed = true;
        true
    });
    assert_eq!(result, RunStatus::Success);
    assert!(!probed);
}

#[test]
fn read_kill_reason_reads_sentinel_only() {
    let dir = std::env::temp_dir().join(format!("moadim-killreason-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();

    assert_eq!(read_kill_reason(&dir), None, "no exit_code file");

    std::fs::write(dir.join("exit_code"), "0\n").unwrap();
    assert_eq!(read_kill_reason(&dir), None, "numeric exit code");

    std::fs::write(dir.join("exit_code"), "killed\n").unwrap();
    assert_eq!(read_kill_reason(&dir), Some(KillReason::MaxRuntime));

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn run_summary_serializes_killed_status_and_reason() {
    use crate::routines::model::RunSummary;
    let run = RunSummary {
        workbench: "demo-1000".into(),
        started_at: 1000,
        started_at_local: String::new(),
        finished_at: Some(1600),
        finished_at_local: None,
        status: RunStatus::Killed,
        exit_code: None,
        kill_reason: Some(KillReason::MaxRuntime),
        retention_expires_at: None,
    };
    let value = serde_json::to_value(&run).unwrap();
    assert_eq!(value["status"], "killed");
    assert_eq!(value["kill_reason"], "max_runtime");
    assert_eq!(value["exit_code"], serde_json::Value::Null);
}
