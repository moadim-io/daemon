//! tmux session side-effects for the cleanup sweep: probing, force-killing, and noting a kill.
//!
//! These are the real-world effects injected into [`super::reap_dir`] (which stays pure and
//! testable). Each is best-effort: a missing `tmux` binary or an already-gone session is never an
//! error, since the only thing that matters is whether a session is running afterwards.

use std::path::Path;

/// The `tmux` executable, overridable via `MOADIM_TMUX_BIN`. In test builds, when no override is
/// set, this resolves to a non-existent path so tmux probes/kills are harmless no-ops and tests
/// never touch the real tmux server. Mirrors the `MOADIM_CRONTAB_BIN` seam (#211).
pub(super) fn tmux_bin() -> String {
    if let Ok(bin) = std::env::var("MOADIM_TMUX_BIN") {
        return bin;
    }
    #[cfg(test)]
    let fallback = "/nonexistent/moadim-test-tmux-guard".to_string();
    #[cfg(not(test))]
    let fallback = "tmux".to_string();
    fallback
}

/// Return `true` if a tmux session named `session` currently exists.
///
/// Uses an exact (`=`) target match so `moadim-foo-1` never matches `moadim-foo-10`. A missing
/// `tmux` binary (exit status unavailable) is treated as "not alive": with no tmux there is no
/// running session to protect, so an expired workbench is safe to reap.
pub(super) fn tmux_session_alive(session: &str) -> bool {
    std::process::Command::new(tmux_bin())
        .arg("has-session")
        .arg("-t")
        .arg(format!("={session}"))
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

/// Force-kill the tmux session named `session` (best-effort).
///
/// Uses an exact (`=`) target match, mirroring [`tmux_session_alive`]. Failures (no `tmux`, session
/// already gone) are ignored: the goal is only that the session is not running afterwards.
pub(super) fn tmux_kill_session(session: &str) {
    let _ = std::process::Command::new(tmux_bin())
        .arg("kill-session")
        .arg("-t")
        .arg(format!("={session}"))
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
}

/// Record a watchdog kill in the run's `agent.log` (best-effort), so an operator reading the log
/// sees why the session ended. `workbench` is the run directory; the note is appended to its
/// `agent.log` (the same file the live session's output is piped to).
pub(super) fn note_forced_kill(workbench: &Path) {
    use std::io::Write;
    let path = workbench.join("agent.log");
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .append(true)
        .create(true)
        .open(path)
    {
        let _ = file.write_all(b"moadim: routine exceeded max runtime; killing session\n");
    }
}
