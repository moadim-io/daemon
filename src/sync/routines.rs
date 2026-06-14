//! Forward synchronization of routines into a dedicated OS crontab block.
//!
//! Routines own a delimited block separate from the handler block:
//!
//! ```text
//! # BEGIN MOADIM-ROUTINES
//! # Managed by moadim — routines (agent tmux sessions)
//! * * * * * TS=$(date +\%s); ...; tmux new-session ... # moadim-routine:<id>
//! # END MOADIM-ROUTINES
//! ```
//!
//! Each line is the full self-contained command produced by [`crate::routines::build_routine_command`]
//! prefixed with the schedule and tagged with the routine id. Reverse sync is not implemented —
//! routines are managed only through the API.

use crate::routines::{
    build_routine_command, load_agent_command, AgentCommand, Routine, RoutineStore,
};
use crate::sync::{read_crontab, replace_block_with, to_os_schedule, write_crontab, SyncError};

/// Delimiter marking the start of the moadim routines crontab block.
const BLOCK_BEGIN: &str = "# BEGIN MOADIM-ROUTINES";
/// Delimiter marking the end of the moadim routines crontab block.
const BLOCK_END: &str = "# END MOADIM-ROUTINES";
/// Human-readable header comment written inside the block.
const BLOCK_HEADER: &str = "# Managed by moadim — routines (agent tmux sessions)";

/// Escape `%` as `\%` so cron does not interpret it as a newline in the command.
fn escape_percent(s: &str) -> String {
    s.replace('%', "\\%")
}

/// Format a single routine as a crontab line: `<schedule> <command> # moadim-routine:<id>`.
pub(crate) fn format_routine_line(routine: &Routine, agent: &AgentCommand) -> String {
    let schedule = to_os_schedule(&routine.schedule);
    let command = escape_percent(&build_routine_command(routine, agent));
    format!("{} {} # moadim-routine:{}", schedule, command, routine.id)
}

/// Build the full routines block from the enabled managed routines in `store`.
///
/// Routines whose agent config is missing are skipped with a warning.
fn build_block(store: &RoutineStore) -> String {
    let mut routines: Vec<Routine> = {
        let lock = store.lock().unwrap();
        lock.values()
            .filter(|r| r.source == "managed" && r.enabled)
            .cloned()
            .collect()
    };
    routines.sort_by_key(|r| r.created_at);

    let lines: Vec<String> = routines
        .iter()
        .filter_map(|r| match load_agent_command(&r.agent) {
            Some(agent) => Some(format_routine_line(r, &agent)),
            None => {
                log::warn!(
                    "routine sync: agent config not found for routine {:?} (agent {:?}); skipping",
                    r.id,
                    r.agent
                );
                None
            }
        })
        .collect();

    if lines.is_empty() {
        format!("{BLOCK_BEGIN}\n{BLOCK_HEADER}\n{BLOCK_END}")
    } else {
        format!(
            "{BLOCK_BEGIN}\n{BLOCK_HEADER}\n{}\n{BLOCK_END}",
            lines.join("\n")
        )
    }
}

/// Write all enabled managed routines from `store` into the OS routines crontab block.
///
/// Idempotent: skips the `crontab -` call when the crontab would not change.
pub fn sync_routines_to_crontab(store: &RoutineStore) -> Result<(), SyncError> {
    let current = read_crontab()?;
    let block = build_block(store);
    let new_crontab = replace_block_with(&current, &block, BLOCK_BEGIN, BLOCK_END);
    if new_crontab == current {
        return Ok(());
    }
    write_crontab(&new_crontab)
}

#[cfg(test)]
#[path = "routines_sync_tests.rs"]
mod routines_sync_tests;
