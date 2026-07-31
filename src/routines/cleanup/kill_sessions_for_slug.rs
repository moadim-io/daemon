#![allow(
    clippy::wildcard_imports,
    clippy::too_many_arguments,
    clippy::needless_pass_by_value,
    dead_code,
    reason = "split-out module preserves existing code while keeping files under the linecheck limit"
)]
use super::*;

/// Force-kill every still-running session under `dir` whose workbench name parses to `slug`,
/// regardless of runtime. Returns the number of sessions killed. `is_alive`/`kill` are injected so
/// the decision logic is unit-testable without a live tmux, mirroring [`watchdog_dir()`].
pub(crate) fn kill_sessions_for_slug(
    dir: &Path,
    slug: &str,
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
        if dir_slug != slug {
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
