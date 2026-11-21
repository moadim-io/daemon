//! Single source of truth for a run's outcome: [`RunStatus`] and [`KillReason`], derived from a
//! workbench's `exit_code` file and the tmux session's liveness.

use std::path::Path;

use crate::routines::model::{KillReason, RunStatus};

/// Content the watchdog writes to a workbench's `exit_code` file when it force-kills the run (see
/// `cleanup::session::note_forced_kill`). Not a number, so it never parses as an exit code.
pub(crate) const KILLED_SENTINEL: &str = "killed";

/// Read the watchdog's kill record from `workbench_path`'s `exit_code` file, if it holds the
/// [`KILLED_SENTINEL`]. The only kill path today is the max-runtime watchdog.
pub(crate) fn read_kill_reason(workbench_path: &Path) -> Option<KillReason> {
    std::fs::read_to_string(workbench_path.join("exit_code"))
        .ok()
        .filter(|text| text.trim() == KILLED_SENTINEL)
        .map(|_| KillReason::MaxRuntime)
}

/// Derive a run's status from its recorded exit code, its kill record, and tmux liveness.
///
/// A recorded exit code wins: `0` is [`RunStatus::Success`], anything else [`RunStatus::Failed`].
/// A kill record without an exit code is [`RunStatus::Killed`] once the session is gone. While the
/// session is still alive after a kill attempt (the tmux kill is best effort), the run stays
/// [`RunStatus::Running`]. With no kill record, a live session is [`RunStatus::Running`] and a gone
/// one is [`RunStatus::Unknown`]. `session_alive` is only evaluated when it is needed.
pub(crate) fn derive_run_status(
    exit_code: Option<i32>,
    kill_reason: Option<KillReason>,
    session_alive: impl FnOnce() -> bool,
) -> RunStatus {
    match (exit_code, kill_reason) {
        (Some(0), _) => RunStatus::Success,
        (Some(_), _) => RunStatus::Failed,
        (None, _) if session_alive() => RunStatus::Running,
        (None, Some(_)) => RunStatus::Killed,
        (None, None) => RunStatus::Unknown,
    }
}

#[cfg(test)]
#[path = "run_status_tests.rs"]
mod run_status_tests;
