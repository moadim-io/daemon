//! Forward synchronization of routines into a dedicated OS crontab block.
//!
//! Routines own a delimited block separate from the handler block:
//!
//! ```text
//! # BEGIN MOADIM-ROUTINES
//! # Managed by moadim — routines (agent tmux sessions)
//! * * * * * /…/moadim schedule trigger '<id>' # moadim-routine:<id>
//! # END MOADIM-ROUTINES
//! ```
//!
//! Each crontab line invokes the `moadim` binary directly to trigger the routine by ID
//! (`moadim schedule trigger <id>`). No per-routine `run.sh` script is generated: the command is
//! short enough to inline (well under cron's ~1000-char per-line limit), and the running daemon is
//! the single source of truth for launch logic ([`crate::routines::build_routine_command`] + spawn).
//! This means **scheduled routines require the daemon to be running** — it is installed as an OS
//! service (launchd / systemd user) for exactly this reason.
//!
//! The binary is referenced by absolute path ([`std::env::current_exe`]) so resolution does not
//! depend on cron's minimal `PATH`. The agent still inherits the user's login environment (`GH_TOKEN`,
//! API keys, …): the daemon's trigger path spawns the agent under `sh -lc`, which sources
//! `~/.profile`. Reverse sync is not implemented — routines are managed only through the API.

use std::sync::{Mutex, OnceLock};

use crate::routine_storage::read_routine_crons;
use crate::routines::{load_agent_command, shell_quote, Routine, RoutineStore};
use crate::sync::{read_crontab, replace_block_with, to_os_schedule, write_crontab, SyncError};
use crate::utils::cron::{normalize_schedule, validate_cron};
use crate::utils::lock::LockRecover;

#[allow(
    clippy::missing_docs_in_private_items,
    reason = "split-out module keeps the file under the linecheck limit"
)]
#[path = "sync_routines_to_crontab.rs"]
mod sync_routines_to_crontab;
pub(crate) use sync_routines_to_crontab::*;

/// Process-wide lock serializing the crontab read-modify-write sequence.
///
/// `sync_routines_to_crontab` is invoked from many concurrent request handlers (REST, MCP) on a
/// multi-threaded runtime. Each call does an unsynchronized `crontab -l` -> edit -> `crontab -`
/// round trip; two calls whose round trips overlap can interleave, and the later `crontab -` wins
/// outright — no merge, no error (issue #365). Taken as the very first thing in
/// `sync_routines_to_crontab`, before the (separate) `RoutineStore` lock, so lock order is always
/// crontab-lock -> store-lock and this can never deadlock against a caller that only takes the
/// store lock.
fn crontab_sync_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

/// Delimiter marking the start of the moadim routines crontab block.
pub(crate) const BLOCK_BEGIN: &str = "# BEGIN MOADIM-ROUTINES";
/// Delimiter marking the end of the moadim routines crontab block.
pub(crate) const BLOCK_END: &str = "# END MOADIM-ROUTINES";
/// Human-readable header comment written inside the block.
const BLOCK_HEADER: &str = "# Managed by moadim — routines (agent tmux sessions)";

/// Format a single routine as a crontab line that triggers it via the `moadim` binary:
/// `<schedule> '<moadim>' schedule trigger '<id>' # moadim-routine:<id>`.
///
/// The binary is referenced by absolute path ([`std::env::current_exe`]) so cron's minimal `PATH`
/// cannot break resolution; both the path and the routine ID are shell-quoted. The launch command
/// itself ([`crate::routines::build_routine_command`]) is built and spawned by the daemon when the
/// `schedule trigger` request arrives, so it is not duplicated into the crontab line.
pub(crate) fn format_routine_line_for_schedule(routine: &Routine, schedule: &str) -> String {
    // The daemon is already running from this binary, so resolving its own path cannot realistically
    // fail; a failure here means the process has no executable path at all, which is unrecoverable.
    #[allow(
        clippy::expect_used,
        reason = "the daemon is already running from this binary, so resolving its own path \
                  cannot realistically fail; a failure here means the process has no executable \
                  path at all, which is unrecoverable, and this fn has no `Result` to propagate \
                  through short of reshaping every crontab-formatting caller"
    )]
    let exe = std::env::current_exe().expect("daemon executable path is resolvable");
    let schedule = to_os_schedule(schedule);
    format!(
        "{} {} schedule trigger {} # moadim-routine:{}",
        schedule,
        shell_quote(&exe.to_string_lossy()),
        shell_quote(&routine.id),
        routine.id
    )
}

/// Format a single routine using its public schedule field.
#[cfg(test)]
pub(crate) fn format_routine_line(routine: &Routine) -> String {
    format_routine_line_for_schedule(routine, &routine.schedule)
}

/// Read the pure cron sidecar schedules for a routine, falling back to the loaded single schedule.
fn pure_schedules_for_crontab(routine: &Routine) -> Vec<String> {
    let rel_dir = crate::routine_storage::routine_rel_dir(routine);
    let entries = read_routine_crons(&crate::paths::routine_dir(&rel_dir).join("schedule.cron"));
    if entries.is_empty() {
        vec![routine.schedule.clone()]
    } else {
        entries
    }
}

/// Compile pure schedules with cron-union, skipping invalid human-edited entries.
fn compailed_schedules_for_crontab(routine: &Routine, pure_schedules: &[String]) -> Vec<String> {
    let schedules: Vec<String> = pure_schedules
        .iter()
        .map(|entry| normalize_schedule(entry))
        .filter(|schedule| match validate_cron(schedule) {
            Ok(()) => true,
            Err(err) => {
                log::warn!(
                    "routine sync: invalid schedule.cron entry {:?} for routine {:?}: {}; skipping",
                    schedule,
                    routine.id,
                    err
                );
                false
            }
        })
        .collect();
    if schedules.is_empty() {
        return vec![routine.schedule.clone()];
    }
    let refs: Vec<&str> = schedules.iter().map(String::as_str).collect();
    match cron_union::union(refs) {
        Ok(union) => union.iter().map(ToString::to_string).collect(),
        Err(err) => {
            log::warn!(
                "routine sync: cron-union could not compile schedules for routine {:?}: {}; \
                 using validated schedules without dedupe",
                routine.id,
                err
            );
            schedules
        }
    }
}

/// Write the gitignored cron-union output sidecar used by the OS crontab.
fn write_compailed_cron_sidecar(routine: &Routine, schedules: &[String]) {
    let rel_dir = crate::routine_storage::routine_rel_dir(routine);
    let dir = crate::paths::routine_dir(&rel_dir);
    let path = crate::paths::routine_compailed_cron_path(&rel_dir);
    let legacy_path = dir.join(".compailed.cron");
    let mut text = schedules.join("\n");
    text.push('\n');
    let _ = crate::utils::fs_perms::create_private_dir_all(&dir);
    let _ = std::fs::write(&path, text);
    if legacy_path != path && legacy_path.exists() {
        let _ = std::fs::remove_file(legacy_path);
    }
}
include!("build_block.rs");
