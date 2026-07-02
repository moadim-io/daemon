//! tmux session side-effects for the cleanup sweep: probing, force-killing, and noting a kill.
//!
//! These are the real-world effects injected into [`super::reap_dir`] (which stays pure and
//! testable). Each is best-effort: a missing `tmux` binary or an already-gone session is never an
//! error, since the only thing that matters is whether a session is running afterwards.

use std::path::Path;

/// The `tmux` executable, overridable via `MOADIM_TMUX_BIN`. In test builds, when no override is
/// set, this resolves to a non-existent path so tmux probes/kills are harmless no-ops and tests
/// never touch the real tmux server. Mirrors the `MOADIM_CRONTAB_BIN` seam (#211).
///
/// Outside tests, resolves via [`super::super::command::resolve_tmux_bin`] rather than the bare
/// `"tmux"` name: launchd/systemd start the daemon with a minimal `PATH` that hides a Homebrew- or
/// npm-installed `tmux`, which used to make every probe here silently fail (read as "session
/// dead") — TTL-reaping a hung run's workbench while its tmux session and agent process kept
/// running, untracked, forever.
pub(super) fn tmux_bin() -> String {
    if let Ok(bin) = std::env::var("MOADIM_TMUX_BIN") {
        return bin;
    }
    #[cfg(test)]
    let fallback = "/nonexistent/moadim-test-tmux-guard".to_string();
    #[cfg(not(test))]
    let fallback = super::super::command::resolve_tmux_bin();
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
        .is_ok_and(|status| status.success())
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

/// Record a watchdog kill in the run's `agent.log` *and* its `exit_code` file (best-effort).
///
/// `workbench` is the run directory. The human-readable note is appended to `agent.log` (the same
/// file the live session's output is piped to) so an operator reading the log sees why the session
/// ended. The machine-readable `killed` sentinel is written to `exit_code`, the same file a
/// normally-finishing run writes its numeric `$?` into (see `command::build_routine_command`); the
/// distinct sentinel keeps a watchdog-killed run from masquerading as a clean `0` exit. The kill
/// SIGKILLs the agent's pane before its own `echo $? > exit_code` can run, so there is no clobber.
pub(super) fn note_forced_kill(workbench: &Path) {
    use std::io::Write;
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .append(true)
        .create(true)
        .open(workbench.join("agent.log"))
    {
        let _ = file.write_all(b"moadim: routine exceeded max runtime; killing session\n");
    }
    let _ = std::fs::write(workbench.join("exit_code"), b"killed\n");
}
