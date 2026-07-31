#![allow(
    clippy::wildcard_imports,
    clippy::too_many_arguments,
    clippy::needless_pass_by_value,
    dead_code,
    reason = "split-out module preserves existing code while keeping files under the linecheck limit"
)]
use super::*;

#[test]
fn write_crontab_errors_instead_of_panicking_on_broken_pipe() {
    // The shim's `-` branch never reads stdin and exits immediately, so
    // writing content larger than the OS pipe buffer must observe a broken
    // pipe. Before this test, that write failure was `.expect()`'d into a
    // panic instead of being returned as a `SyncError`.
    let shim = CronShim::write_pipe_closed();
    let big_content = "x".repeat(4 * 1024 * 1024);
    let err = write_crontab(&big_content).unwrap_err();
    match err {
        SyncError::Io(_) => {}
        SyncError::CrontabCommand(msg) => {
            panic!("expected Io error from the broken pipe, got CrontabCommand({msg})")
        }
    }
    drop(shim);
}

#[test]
fn write_crontab_times_out_and_kills_hung_install() {
    let _timeout = EnvGuard::set(CRONTAB_WRITE_TIMEOUT_ENV, "1");
    let shim = CronShim::write_hangs();
    let err = write_crontab("anything\n").unwrap_err();
    match err {
        SyncError::CrontabCommand(msg) => {
            assert!(msg.contains("timed out after 1s"), "unexpected msg: {msg}");
            assert!(msg.contains("killed pid"), "missing kill detail: {msg}");
        }
        SyncError::Io(io) => panic!("expected CrontabCommand, got Io({io})"),
    }
    drop(shim);
}

#[test]
fn write_crontab_errors_when_binary_is_missing() {
    // Pointing the crontab seam at a nonexistent binary makes the spawn fail, exercising the
    // spawn-failure error branch.
    let previous = std::env::var_os("MOADIM_CRONTAB_BIN");
    // SAFETY: single-threaded test execution.
    unsafe {
        std::env::set_var(
            "MOADIM_CRONTAB_BIN",
            "/nonexistent/moadim-no-such-crontab-xyz",
        );
    }
    let result = write_crontab("# BEGIN MOADIM-ROUTINES\n# END MOADIM-ROUTINES\n");
    // SAFETY: single-threaded test execution.
    unsafe {
        match previous {
            Some(value) => std::env::set_var("MOADIM_CRONTAB_BIN", value),
            None => std::env::remove_var("MOADIM_CRONTAB_BIN"),
        }
    }
    assert!(
        result.is_err(),
        "spawning a missing crontab binary must error"
    );
}
