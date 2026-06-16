//! Replace an already-running daemon with a fresh process.
//!
//! When a launch finds a server already responding on [`crate::cli::BIND_ADDR`], we stop it and
//! start a new instance instead of reusing it, so each launch yields a clean process.

use std::time::Duration;

use crate::cli::{is_running, read_pid_file, BIND_ADDR};

/// How long to wait for an already-running server to exit before starting a fresh one.
const RESTART_TIMEOUT: Duration = Duration::from_secs(5);

/// How often to re-probe the port while waiting for the old server to exit.
const POLL_INTERVAL: Duration = Duration::from_millis(100);

/// Stop the running server and block until it stops answering, falling back to a kill signal.
///
/// Sends `POST /shutdown` for a graceful exit, then polls [`is_running`] until the port goes quiet
/// or [`RESTART_TIMEOUT`] elapses. If it is still up by then, the recorded PID is killed directly.
pub fn stop_running_and_wait() -> anyhow::Result<()> {
    let _ = crate::cli::http_request("POST", "/shutdown");

    if wait_until_stopped() {
        return Ok(());
    }

    // Graceful shutdown did not take effect in time; force-kill the recorded PID.
    if let Some(pid) = read_pid_file() {
        kill_pid(pid);
        if wait_until_stopped() {
            return Ok(());
        }
    }

    if is_running() {
        anyhow::bail!("could not stop the running moadim instance at http://{BIND_ADDR}");
    }
    Ok(())
}

/// Poll the port until it stops answering or [`RESTART_TIMEOUT`] elapses. Returns `true` if stopped.
fn wait_until_stopped() -> bool {
    let deadline = std::time::Instant::now() + RESTART_TIMEOUT;
    while std::time::Instant::now() < deadline {
        if !is_running() {
            return true;
        }
        std::thread::sleep(POLL_INTERVAL);
    }
    !is_running()
}

/// Force-kill a process by PID. Best-effort: a missing/already-dead process is ignored.
#[cfg(unix)]
fn kill_pid(pid: u32) {
    let _ = std::process::Command::new("kill")
        .args(["-9", &pid.to_string()])
        .output();
}

/// Force-kill a process by PID. Best-effort: failures are ignored.
#[cfg(not(unix))]
fn kill_pid(pid: u32) {
    let _ = std::process::Command::new("taskkill")
        .args(["/F", "/PID", &pid.to_string()])
        .output();
}
