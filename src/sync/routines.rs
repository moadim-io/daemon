//! Forward synchronization of routines into a dedicated OS crontab block.
//!
//! Routines own a delimited block separate from the handler block:
//!
//! ```text
//! # BEGIN MOADIM-ROUTINES
//! # Managed by moadim — routines (agent tmux sessions)
//! * * * * * /bin/sh '/…/routines/<slug>/run.sh' # moadim-routine:<id>
//! # END MOADIM-ROUTINES
//! ```
//!
//! Each routine's full launch command is written to a per-routine `run.sh` script; the crontab line
//! is just `<schedule> /bin/sh '<run.sh>' # moadim-routine:<id>`. Inlining the whole command (which
//! includes a long per-agent `setup` step) pushed lines past cron's ~1000-char per-line limit, which
//! cron silently drops. Reverse sync is not implemented — routines are managed only through the API.

use std::io;
use std::os::unix::fs::PermissionsExt;

use crate::paths::routine_script_path;
use crate::routines::{
    build_routine_command, load_agent_command, shell_quote, slugify, AgentCommand, Routine,
    RoutineStore,
};
use crate::sync::{read_crontab, replace_block_with, to_os_schedule, write_crontab, SyncError};

/// Delimiter marking the start of the moadim routines crontab block.
const BLOCK_BEGIN: &str = "# BEGIN MOADIM-ROUTINES";
/// Delimiter marking the end of the moadim routines crontab block.
const BLOCK_END: &str = "# END MOADIM-ROUTINES";
/// Human-readable header comment written inside the block.
const BLOCK_HEADER: &str = "# Managed by moadim — routines (agent tmux sessions)";

/// Write the routine's launch command to its `run.sh` script and return the path.
///
/// The script holds the full self-contained command from [`build_routine_command`], so the crontab
/// line that calls it stays short regardless of how long the command is.
fn write_routine_script(routine: &Routine, agent: &AgentCommand) -> io::Result<std::path::PathBuf> {
    let path = routine_script_path(&slugify(&routine.title));
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let command = build_routine_command(routine, agent);
    std::fs::write(&path, format!("#!/bin/sh\n{command}\n"))?;
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))?;
    Ok(path)
}

/// Format a single routine as a crontab line that invokes its `run.sh`:
/// `<schedule> /bin/sh '<run.sh>' # moadim-routine:<id>`.
///
/// Returns `None` (after a warning) if the script cannot be written.
pub(crate) fn format_routine_line(routine: &Routine, agent: &AgentCommand) -> Option<String> {
    let script = match write_routine_script(routine, agent) {
        Ok(p) => p,
        Err(e) => {
            log::warn!(
                "routine sync: failed to write run.sh for routine {:?}: {e}; skipping",
                routine.id
            );
            return None;
        }
    };
    let schedule = to_os_schedule(&routine.schedule);
    Some(format!(
        "{} /bin/sh {} # moadim-routine:{}",
        schedule,
        shell_quote(&script.to_string_lossy()),
        routine.id
    ))
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
            Some(agent) => format_routine_line(r, &agent),
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
