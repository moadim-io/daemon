
/// Ensure the config dir `.gitignore` contains all required patterns on every start.
///
/// This is the single `.gitignore` for the whole config tree — its patterns apply recursively, so
/// they also cover every routine directory (per-routine `.gitignore` files are no longer
/// generated): `*.local.*` catches the machine-local sidecars (`state.local.toml`,
/// `routine.local.toml`, `prompt.compiled.local.md`), `*.log` the trigger logs, `*.compiled.*`
/// the legacy `prompts/prompt.compiled.md`, `schedule.compailed.cron` the cron-union output,
/// `.compailed.cron` the legacy cron-union output name, and `run.sh` the obsolete per-routine
/// launch script.
///
/// Reads the existing file (if any), appends any missing patterns, and writes back only when
/// something changed. Preserves user-added entries. Best-effort: failure is not fatal.
fn ensure_config_gitignore() {
    const REQUIRED: &[&str] = &[
        "*.pid",
        "*.log",
        "*.local.*",
        "*.compiled.*",
        "schedule.compailed.cron",
        ".compailed.cron",
        "run.sh",
    ];
    let gitignore = crate::paths::config_gitignore_path();
    let existing = std::fs::read_to_string(&gitignore).unwrap_or_default();
    let lines: Vec<&str> = existing.lines().collect();
    let missing: Vec<&str> = REQUIRED
        .iter()
        .copied()
        .filter(|pat| !lines.iter().any(|line| line.trim() == *pat))
        .collect();
    if missing.is_empty() {
        return;
    }
    let mut content = existing;
    if !content.is_empty() && !content.ends_with('\n') {
        content.push('\n');
    }
    for pattern in &missing {
        content.push_str(pattern);
        content.push('\n');
    }
    let _ = std::fs::write(&gitignore, &content);
}

/// Remove the pid file. Best-effort: a missing file is not an error.
pub fn clear_pid_file() {
    let _ = std::fs::remove_file(crate::paths::pid_file());
}

/// Read the PID recorded in the pid file, if present, parseable, **and still a live process**.
///
/// On a clean shutdown the pid file is removed ([`clear_pid_file`]), but a `kill -9`, panic, OOM
/// kill, or power loss skips that path and leaves the file behind with a now-dead PID. Returning
/// that stale PID would make the machine-readable contract dishonest — `status`/`stop --json` would
/// report a `pid` for a process that no longer exists (and which, after PID reuse, may belong to an
/// unrelated process), and `restart` would force-kill it. So a recorded PID that is not alive is
/// treated as absent and the stale file is cleaned up best-effort, keeping the pid self-healing.
pub(crate) fn read_pid_file() -> Option<u32> {
    let pid = std::fs::read_to_string(crate::paths::pid_file())
        .ok()?
        .trim()
        .parse()
        .ok()?;
    if process_is_alive(pid) {
        Some(pid)
    } else {
        clear_pid_file();
        None
    }
}

/// Returns `true` if a process with `pid` currently exists.
///
/// Uses the standard signal-0 liveness probe (`kill -0 <pid>`): no signal is delivered, but the
/// kernel runs the same existence check it would for a real signal, so a successful exit means the
/// PID is alive. Unlike [`crate::restart`]'s destructive killer, this probe is harmless, so it goes
/// straight to the real `kill` with no env override to keep parallel tests from racing on it.
#[cfg(unix)]
fn process_is_alive(pid: u32) -> bool {
    // Linux PIDs are signed; u32::MAX overflows to -1 in the kernel, causing kill(-1, 0)
    // to return success (it sends to all accessible processes). Treat out-of-range as dead.
    if pid > i32::MAX as u32 {
        return false;
    }
    std::process::Command::new("kill")
        .args(["-0", &pid.to_string()])
        .output()
        .is_ok_and(|out| out.status.success())
}

/// Best-effort liveness on non-unix: assume the recorded PID is alive, mirroring the best-effort
/// semantics of the Windows force-killer. The pid file is still cleared on graceful shutdown.
#[cfg(not(unix))]
fn process_is_alive(_pid: u32) -> bool {
    true
}

/// The daemon log path, rendered for display.
pub(super) fn paths_daemon_log() -> String {
    crate::paths::daemon_log_file().display().to_string()
}

/// Returns `true` if a server answers `GET /health` on [`super::BIND_ADDR`].
pub(crate) fn is_running() -> bool {
    matches!(http_request("GET", "/api/v1/health"), Ok(200))
}

/// Interval between liveness probes while [`wait_until`] polls for a server to come up.
const WAIT_POLL_INTERVAL: Duration = Duration::from_millis(200);

#[cfg(test)]
#[path = "system_tests.rs"]
mod cli_system_tests;
