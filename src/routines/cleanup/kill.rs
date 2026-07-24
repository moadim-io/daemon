//! Force-killing routine tmux sessions on demand — deleted routines and daemon shutdown — split
//! out of `cleanup/mod.rs` to keep that file under the line-count gate.

use std::path::Path;

use crate::paths::workbenches_dir;

use super::parse_workbench_name;
use super::session::{tmux_kill_session, tmux_session_alive};

/// Force-kill every still-running session under `dir` whose workbench slug satisfies `matches`,
/// regardless of runtime. Returns the number of sessions killed. `is_alive`/`kill` are injected so
/// the decision logic is unit-testable without a live tmux, mirroring `watchdog_dir`. Shared by
/// [`kill_sessions_for_slug`] (one routine's slug) and [`kill_all_routine_sessions`] (every slug, on
/// daemon shutdown) so the "session name is `moadim-{workbench dir name}`" convention is defined
/// exactly once.
fn kill_matching_sessions(
    dir: &Path,
    matches: &dyn Fn(&str) -> bool,
    is_alive: &dyn Fn(&str) -> bool,
    kill: &dyn Fn(&str),
) -> usize {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return 0;
    };
    let mut killed = 0;
    for entry in entries.flatten() {
        if !entry.file_type().is_ok_and(|ft| ft.is_dir()) {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        let Some((dir_slug, _ts)) = parse_workbench_name(&name) else {
            continue;
        };
        if !matches(dir_slug) {
            continue;
        }
        let session = format!("moadim-{name}");
        if is_alive(&session) {
            kill(&session);
            killed += 1;
        }
    }
    killed
}

/// Force-kill every still-running session under `dir` whose workbench name parses to `slug`,
/// regardless of runtime. Returns the number of sessions killed. `is_alive`/`kill` are injected so
/// the decision logic is unit-testable without a live tmux, mirroring `watchdog_dir`.
fn kill_sessions_for_slug(
    dir: &Path,
    slug: &str,
    is_alive: &dyn Fn(&str) -> bool,
    kill: &dyn Fn(&str),
) -> usize {
    kill_matching_sessions(dir, &|dir_slug| dir_slug == slug, is_alive, kill)
}

/// Kill any still-running workbench session(s) belonging to a just-deleted routine's `slug`.
///
/// Without this, deleting a routine while its agent is mid-run left that run executing
/// unsupervised: the workbench and its tmux session survived until the next TTL sweep reaped the
/// now-orphaned workbench, up to `effective_ttl_secs` later (issue #333). The workbench directory
/// itself is left untouched here — it is removed by the caller (or reaped normally otherwise).
/// Returns the number of sessions killed.
pub fn kill_sessions_for_deleted_routine(slug: &str) -> usize {
    kill_sessions_for_slug(
        &workbenches_dir(),
        slug,
        &tmux_session_alive,
        &tmux_kill_session,
    )
}

/// Force-kill every still-live routine tmux session under `~/.moadim/workbenches/`, regardless of
/// which routine it belongs to.
///
/// Called once from the shutdown path (`moadim stop` / the UI STOP button / `POST /shutdown`,
/// see `routes::http_listener::run_with_listener_until`) so an in-flight agent doesn't outlive the
/// daemon that launched it: routine agents run in a **detached** tmux session, independent of the
/// daemon process, so previously nothing but the next start's watchdog/cleanup sweep ever reaped
/// them — an operator who believed `moadim stop` had stopped everything could have an agent keep
/// acting on their behalf (opening PRs, pushing commits) for as long as it kept running (#320).
///
/// Reuses the exact same session-naming convention (`moadim-{workbench dir name}`) and
/// alive/kill probes as [`kill_sessions_for_deleted_routine`] and the watchdog above, rather than
/// inventing a second way to enumerate routine sessions. `tmux_session_alive`/`tmux_kill_session`
/// already treat a missing `tmux` binary or an already-gone session as a no-op, so shutdown never
/// fails because of tmux. Returns the number of sessions killed.
pub fn kill_all_routine_sessions() -> usize {
    kill_matching_sessions(
        &workbenches_dir(),
        &|_dir_slug| true,
        &tmux_session_alive,
        &tmux_kill_session,
    )
}

#[cfg(test)]
#[path = "kill_tests.rs"]
mod kill_tests;
