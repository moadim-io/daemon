//! Forward synchronization of routines into a dedicated OS crontab block.
//!
//! Routines own a delimited block separate from the handler block:
//!
//! ```text
//! # BEGIN MOADIM-ROUTINES
//! # Managed by moadim — routines (agent tmux sessions)
//! * * * * * /bin/sh -l '/…/routines/<slug>/run.sh' # moadim-routine:<id>
//! # END MOADIM-ROUTINES
//! ```
//!
//! Each routine's full launch command is written to a per-routine `run.sh` script; the crontab line
//! is just `<schedule> /bin/sh -l '<run.sh>' # moadim-routine:<id>`. Inlining the whole command
//! (which includes a long per-agent `setup` step) pushed lines past cron's ~1000-char per-line
//! limit, which cron silently drops.
//!
//! The `-l` makes `run.sh` execute under a **login** shell so it sources the user's `~/.profile`,
//! giving the agent the environment variables (`GH_TOKEN`, API keys, …) an interactive login shell
//! has. Cron's own environment is minimal and outside the GUI login session, so without `-l` the
//! agent would launch with no credentials. (`run.sh` still replaces `PATH` with its own curated
//! list, so binary resolution is unaffected — only env vars are gained.) Reverse sync is not
//! implemented — routines are managed only through the API.

use crate::utils::lock::LockRecover;
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
    std::fs::create_dir_all(path.parent().expect("routine script path has a parent dir"))?;
    let command = build_routine_command(routine, agent);
    std::fs::write(&path, format!("#!/bin/sh\n{command}\n"))?;
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))?;
    Ok(path)
}

/// Format a single routine as a crontab line that invokes its `run.sh`:
/// `<schedule> /bin/sh -l '<run.sh>' # moadim-routine:<id>`.
///
/// The `-l` runs the script under a login shell so it sources the user's `~/.profile`, letting the
/// agent inherit their environment variables (`GH_TOKEN`, API keys, …) rather than cron's minimal
/// one.
///
/// Returns `None` (after a warning) if the script cannot be written.
pub(crate) fn format_routine_line(routine: &Routine, agent: &AgentCommand) -> Option<String> {
    let script = match write_routine_script(routine, agent) {
        Ok(path) => path,
        Err(err) => {
            log::warn!(
                "routine sync: failed to write run.sh for routine {:?}: {err}; skipping",
                routine.id
            );
            return None;
        }
    };
    let schedule = to_os_schedule(&routine.schedule);
    Some(format!(
        "{} /bin/sh -l {} # moadim-routine:{}",
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
        let lock = store.lock_recover();
        lock.values()
            .filter(|routine| routine.source == "managed" && routine.enabled)
            .cloned()
            .collect()
    };
    routines.sort_by_key(|routine| routine.created_at);

    let lines: Vec<String> = routines
        .iter()
        .filter_map(|routine| match load_agent_command(&routine.agent) {
            Ok(agent) => format_routine_line(routine, &agent),
            Err(err) => {
                log::warn!(
                    "routine sync: cannot load agent {:?} ({}) for routine {:?}; skipping",
                    routine.agent,
                    err,
                    routine.id
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

/// Substring identifying a routine line inside the crontab block (`# moadim-routine:<id>`).
const ROUTINE_LINE_MARKER: &str = "# moadim-routine:";

/// Write all enabled managed routines from `store` into the OS routines crontab block.
///
/// Idempotent: skips the `crontab -` call when the crontab would not change.
///
/// Footgun guard: refuses to overwrite a populated routines block when the store is *empty*. An
/// empty store at sync time means the store never loaded (or a second daemon is racing this one),
/// not a genuine "no routines" state — startup always reseeds the built-in defaults, so the steady
/// state is never an empty store. Without this guard such a sync would write a bare block and
/// silently drop every scheduled routine's cron line (the incident that motivated it). A store that
/// loaded fine but holds only disabled/unmanaged routines is *not* empty, so legitimately clearing
/// the last routine still works.
pub fn sync_routines_to_crontab(store: &RoutineStore) -> Result<(), SyncError> {
    let current = read_crontab()?;
    if store.lock_recover().is_empty() && current.contains(ROUTINE_LINE_MARKER) {
        log::warn!(
            "routine sync: store is empty but the crontab still has routine lines; refusing to \
             wipe the routines block (suspected load failure or a concurrent daemon)"
        );
        return Ok(());
    }
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
