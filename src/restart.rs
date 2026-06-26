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

/// Env override for [`RESTART_TIMEOUT`] in milliseconds (test seam): lets tests drive the
/// force-kill/timeout path quickly instead of waiting the full default.
const RESTART_TIMEOUT_MS_ENV: &str = "MOADIM_RESTART_TIMEOUT_MS";

/// Env override for [`POLL_INTERVAL`] in milliseconds (test seam).
const POLL_INTERVAL_MS_ENV: &str = "MOADIM_RESTART_POLL_MS";

/// The stop-wait deadline, honoring [`RESTART_TIMEOUT_MS_ENV`] when set.
fn restart_timeout() -> Duration {
    parse_millis_env(RESTART_TIMEOUT_MS_ENV).unwrap_or(RESTART_TIMEOUT)
}

/// The port re-probe interval, honoring [`POLL_INTERVAL_MS_ENV`] when set.
fn poll_interval() -> Duration {
    parse_millis_env(POLL_INTERVAL_MS_ENV).unwrap_or(POLL_INTERVAL)
}

/// Parse a millisecond [`Duration`] from environment variable `name`, or `None` when unset/invalid.
fn parse_millis_env(name: &str) -> Option<Duration> {
    std::env::var(name)
        .ok()?
        .parse::<u64>()
        .ok()
        .map(Duration::from_millis)
}

/// Stop the running server and block until it stops answering, falling back to a kill signal.
///
/// Sends `POST /shutdown` for a graceful exit, then polls [`is_running`] until the port goes quiet
/// or [`RESTART_TIMEOUT`] elapses. If it is still up by then, the recorded PID is killed directly.
pub fn stop_running_and_wait() -> anyhow::Result<()> {
    let _ = crate::cli::http_request("POST", "/api/v1/shutdown");

    if wait_until_stopped() {
        return Ok(());
    }

    // Graceful shutdown did not take effect in time; force-kill the recorded PID, then re-check.
    if let Some(pid) = read_pid_file() {
        kill_pid(pid);
    }

    if wait_until_stopped() {
        Ok(())
    } else {
        anyhow::bail!("could not stop the running moadim instance at http://{BIND_ADDR}")
    }
}

/// Poll the port until it stops answering or [`RESTART_TIMEOUT`] elapses. Returns `true` if stopped.
fn wait_until_stopped() -> bool {
    let deadline = std::time::Instant::now() + restart_timeout();
    while std::time::Instant::now() < deadline {
        if !is_running() {
            return true;
        }
        std::thread::sleep(poll_interval());
    }
    !is_running()
}

/// The process-kill executable. Overridable via `MOADIM_KILL_BIN` so a test can inject a no-op
/// shim instead of signalling a real PID. Defaults to the platform killer (`kill` / `taskkill`);
/// unlike the crontab/tmux seams it does NOT default-deny under cfg(test), because
/// `kill_pid_terminates_a_live_process` exercises a real kill against its own spawned child.
#[cfg(unix)]
fn kill_bin() -> String {
    std::env::var("MOADIM_KILL_BIN").unwrap_or_else(|_| "kill".to_string())
}
#[cfg(not(unix))]
fn kill_bin() -> String {
    std::env::var("MOADIM_KILL_BIN").unwrap_or_else(|_| "taskkill".to_string())
}

/// Force-kill a process by PID. Best-effort: a missing/already-dead process is ignored.
#[cfg(unix)]
fn kill_pid(pid: u32) {
    let _ = std::process::Command::new(kill_bin())
        .args(["-9", &pid.to_string()])
        .output();
}

/// Force-kill a process by PID. Best-effort: failures are ignored.
#[cfg(not(unix))]
fn kill_pid(pid: u32) {
    let _ = std::process::Command::new(kill_bin())
        .args(["/F", "/PID", &pid.to_string()])
        .output();
}

#[cfg(test)]
#[path = "restart_tests.rs"]
mod restart_tests;
