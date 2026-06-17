//! Forward synchronization of routines into the OS scheduler.
//!
//! Each routine's full launch command is written to a per-routine script (`run.sh` on Unix,
//! `run.ps1` on Windows). The scheduler entry just invokes that script, so it stays short regardless
//! of how long the command is.
//!
//! **Unix** owns a delimited crontab block separate from the handler block:
//!
//! ```text
//! # BEGIN MOADIM-ROUTINES
//! # Managed by moadim — routines (agent tmux sessions)
//! * * * * * /bin/sh '/…/routines/<slug>/run.sh' # moadim-routine:<id>
//! # END MOADIM-ROUTINES
//! ```
//!
//! **Windows** registers one Task Scheduler task per routine (`moadim-routine-<id>`) that runs the
//! routine's `run.ps1` via PowerShell.
//!
//! Reverse sync is not implemented on either platform — routines are managed only through the API.

use std::io;

use crate::paths::routine_script_path;
use crate::routines::{
    build_routine_command, load_agent_command, slugify, AgentCommand, Routine, RoutineStore,
};
use crate::sync::SyncError;

#[cfg(unix)]
use crate::routines::shell_quote;
#[cfg(unix)]
use crate::sync::{read_crontab, replace_block_with, to_os_schedule, write_crontab};

/// Delimiter marking the start of the moadim routines crontab block.
#[cfg(unix)]
const BLOCK_BEGIN: &str = "# BEGIN MOADIM-ROUTINES";
/// Delimiter marking the end of the moadim routines crontab block.
#[cfg(unix)]
const BLOCK_END: &str = "# END MOADIM-ROUTINES";
/// Human-readable header comment written inside the block.
#[cfg(unix)]
const BLOCK_HEADER: &str = "# Managed by moadim — routines (agent tmux sessions)";

/// Write the routine's launch command to its script (`run.sh`/`run.ps1`) and return the path.
///
/// The script holds the full self-contained command from [`build_routine_command`]. On Unix it is a
/// `/bin/sh` script marked executable; on Windows it is a `run.ps1` PowerShell script.
fn write_routine_script(routine: &Routine, agent: &AgentCommand) -> io::Result<std::path::PathBuf> {
    let path = routine_script_path(&slugify(&routine.title));
    std::fs::create_dir_all(path.parent().expect("routine script path has a parent dir"))?;
    let command = build_routine_command(routine, agent);
    #[cfg(unix)]
    {
        std::fs::write(&path, format!("#!/bin/sh\n{command}\n"))?;
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))?;
    }
    #[cfg(not(unix))]
    {
        std::fs::write(&path, command)?;
    }
    Ok(path)
}

// ─── Unix (crontab) backend ──────────────────────────────────────────────────

/// Format a single routine as a crontab line that invokes its `run.sh`:
/// `<schedule> /bin/sh '<run.sh>' # moadim-routine:<id>`.
///
/// Returns `None` (after a warning) if the script cannot be written.
#[cfg(unix)]
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
        "{} /bin/sh {} # moadim-routine:{}",
        schedule,
        shell_quote(&script.to_string_lossy()),
        routine.id
    ))
}

/// Build the full routines crontab block from the enabled managed routines in `store`.
///
/// Routines whose agent config is missing are skipped with a warning.
#[cfg(unix)]
fn build_block(store: &RoutineStore) -> String {
    let mut routines: Vec<Routine> = {
        let lock = store.lock().unwrap();
        lock.values()
            .filter(|routine| routine.source == "managed" && routine.enabled)
            .cloned()
            .collect()
    };
    routines.sort_by_key(|routine| routine.created_at);

    let lines: Vec<String> = routines
        .iter()
        .filter_map(|routine| match load_agent_command(&routine.agent) {
            Some(agent) => format_routine_line(routine, &agent),
            None => {
                log::warn!(
                    "routine sync: agent config not found for routine {:?} (agent {:?}); skipping",
                    routine.id,
                    routine.agent
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
#[cfg(unix)]
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
#[cfg(unix)]
pub fn sync_routines_to_crontab(store: &RoutineStore) -> Result<(), SyncError> {
    let current = read_crontab()?;
    if store.lock().unwrap().is_empty() && current.contains(ROUTINE_LINE_MARKER) {
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

// ─── Windows (Task Scheduler) backend ────────────────────────────────────────

/// Reconcile one Windows Task Scheduler task per enabled managed routine.
///
/// Each routine's `run.ps1` is (re)written, then a `moadim-routine-<id>` task is registered to run
/// it via PowerShell. Routines whose agent config is missing, or whose script can't be written, are
/// skipped with a warning.
#[cfg(windows)]
pub fn sync_routines_to_crontab(store: &RoutineStore) -> Result<(), SyncError> {
    let mut routines: Vec<Routine> = {
        let lock = store.lock().unwrap();
        lock.values()
            .filter(|routine| routine.source == "managed" && routine.enabled)
            .cloned()
            .collect()
    };
    routines.sort_by_key(|routine| routine.created_at);

    let mut tasks: Vec<crate::platform::SchedTask> = Vec::new();
    for routine in &routines {
        let Some(agent) = load_agent_command(&routine.agent) else {
            log::warn!(
                "routine sync: agent config not found for routine {:?} (agent {:?}); skipping",
                routine.id,
                routine.agent
            );
            continue;
        };
        let script = match write_routine_script(routine, &agent) {
            Ok(path) => path,
            Err(err) => {
                log::warn!(
                    "routine sync: failed to write run.ps1 for routine {:?}: {err}; skipping",
                    routine.id
                );
                continue;
            }
        };
        tasks.push(crate::platform::SchedTask {
            name: format!("{}{}", crate::platform::ROUTINE_PREFIX, routine.id),
            schedule: routine.schedule.clone(),
            run: crate::platform::routine_run_command(&script),
        });
    }

    crate::platform::reconcile(crate::platform::ROUTINE_PREFIX, &tasks)
}

// Exercises the Unix crontab routines block; gated to Unix where that machinery is compiled.
#[cfg(all(test, unix))]
#[path = "routines_sync_tests.rs"]
mod routines_sync_tests;
