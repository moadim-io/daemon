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

use crate::routines::{load_agent_command, shell_quote, Routine, RoutineStore};
use crate::sync::{read_crontab, replace_block_with, to_os_schedule, write_crontab, SyncError};
use crate::utils::lock::LockRecover;

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
pub(crate) fn format_routine_line(routine: &Routine) -> String {
    // The daemon is already running from this binary, so resolving its own path cannot realistically
    // fail; a failure here means the process has no executable path at all, which is unrecoverable.
    let exe = std::env::current_exe().expect("daemon executable path is resolvable");
    let schedule = to_os_schedule(&routine.schedule);
    format!(
        "{} {} schedule trigger {} # moadim-routine:{}",
        schedule,
        shell_quote(&exe.to_string_lossy()),
        shell_quote(&routine.id),
        routine.id
    )
}

/// Build the full routines block from the enabled managed routines in `store`.
///
/// Only routines assigned to *this* machine ([`crate::machine::current_machine`]) are scheduled: a
/// shared config repo can drive different routines on different machines. A routine with an empty
/// `machines` list runs nowhere — these are logged once as dormant so the operator notices an
/// unassigned routine instead of it silently never firing. Routines whose agent config is missing
/// are skipped with a warning.
fn build_block(store: &RoutineStore) -> String {
    if crate::global_lock::is_globally_locked() {
        log::info!("routine sync: global lock active — clearing all routine crontab lines");
        return format!("{BLOCK_BEGIN}\n{BLOCK_HEADER}\n{BLOCK_END}");
    }
    let me = crate::machine::current_machine();
    let mut routines: Vec<Routine> = {
        let lock = store.lock_recover();
        lock.values()
            .filter(|routine| routine.source == "managed" && routine.enabled)
            .cloned()
            .collect()
    };
    warn_dormant_routines(&routines);
    routines.retain(|routine| crate::machine::targets(&routine.machines, &me));
    // The routines come off a `HashMap`, whose iteration order is unspecified, so routines that
    // share a `created_at` (e.g. several seeded or batch-created in the same second) would otherwise
    // emit in an arbitrary, run-to-run order. That churns the generated crontab block across syncs
    // and defeats the `new_crontab == current` idempotency guard below, forcing a needless
    // `crontab -` rewrite. Break ties on the stable routine id so the block is fully deterministic.
    routines.sort_by(|left, right| {
        left.created_at
            .cmp(&right.created_at)
            .then_with(|| left.id.cmp(&right.id))
    });

    let lines: Vec<String> = routines
        .iter()
        .filter_map(|routine| match load_agent_command(&routine.agent) {
            // Validate the agent config at sync time so a broken routine is skipped here rather than
            // failing at fire time; the crontab line itself no longer embeds the agent command.
            Ok(_) => Some(format_routine_line(routine)),
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

/// Log a single warning naming enabled routines with no machine assignment (empty `machines`).
///
/// With "unset targeting = runs nowhere", such routines never schedule on any machine. Surfacing
/// them once at sync time makes that visible (e.g. after an upgrade from a version without
/// targeting) instead of leaving the operator to wonder why a routine never fires.
fn warn_dormant_routines(routines: &[Routine]) {
    let dormant: Vec<&str> = routines
        .iter()
        .filter(|routine| routine.machines.is_empty())
        .map(|routine| routine.title.as_str())
        .collect();
    if !dormant.is_empty() {
        log::warn!(
            "{} enabled routine(s) have no machine assignment and will not be scheduled on any \
             machine: {}; assign with `moadim routines update <id> --machines '[\"<name>\"]'`",
            dormant.len(),
            dormant.join(", ")
        );
    }
}

/// Substring identifying a routine line inside the crontab block (`# moadim-routine:<id>`).
pub(crate) const ROUTINE_LINE_MARKER: &str = "# moadim-routine:";

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
///
/// Every caller is a REST/MCP async request handler running on the multi-thread runtime
/// (`#[tokio::main]`'s default flavor), but the work below — `crontab -l` / `crontab -` subprocess
/// round trips — is blocking (#360). Run inline, it occupies a worker thread for the whole
/// round-trip; a hung `crontab` binary can tie up enough workers to stall unrelated in-flight
/// requests, including `/health`. [`tokio::task::block_in_place`] tells the runtime this thread is
/// about to block so it can hand its other scheduled tasks to a spare worker. It's only valid (and
/// only needed) on a multi-thread runtime — it panics on `current_thread`, which `#[tokio::test]`
/// defaults to — and only inside a runtime at all (plain `#[test]`s call this function directly
/// with none running), so both are checked first; either falls back to running inline exactly as
/// before.
pub fn sync_routines_to_crontab(store: &RoutineStore) -> Result<(), SyncError> {
    let on_multi_thread_runtime = tokio::runtime::Handle::try_current()
        .is_ok_and(|handle| handle.runtime_flavor() == tokio::runtime::RuntimeFlavor::MultiThread);
    if on_multi_thread_runtime {
        tokio::task::block_in_place(|| sync_routines_to_crontab_blocking(store))
    } else {
        sync_routines_to_crontab_blocking(store)
    }
}

/// Blocking body of [`sync_routines_to_crontab`], split out so the wrapper can choose whether to
/// run it via [`tokio::task::block_in_place`].
fn sync_routines_to_crontab_blocking(store: &RoutineStore) -> Result<(), SyncError> {
    let _crontab_guard = crontab_sync_lock().lock_recover();
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
