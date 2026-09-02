//! Manual/scheduled triggers, snooze, cleanup, logs, and flags for routines.

use crate::error::AppError;
use crate::paths::workbenches_dir;
use crate::routine_storage::{append_manual_trigger_log, append_skip_log, write_routine};
use crate::utils::lock::LockRecover;
use crate::utils::time::now_secs;

use crate::routines::agents::load_agent_command;
use crate::routines::cleanup::{
    cleanup_expired_workbenches, parse_workbench_name, tmux_session_count,
    tmux_session_prefix_alive,
};
use crate::routines::command::{
    build_routine_command, inline_prompt_overflow, tmux_session_prefix, TriggerSource,
    TMUX_SESSION_PREFIX,
};
use crate::routines::model::{CleanupResponse, Routine, RoutineStore};
use crate::routines::{max_concurrent_runs, MAX_CONCURRENT_RUNS_ENV};

use super::scheduled_trigger_claim::claim_current_minute;
use super::service_log_tail::{read_log_tail_with_meta, LogWithMeta};

#[allow(
    clippy::missing_docs_in_private_items,
    reason = "split-out module keeps the file under the linecheck limit"
)]
#[path = "spawn_routine_command.rs"]
mod spawn_routine_command;
pub(crate) use spawn_routine_command::*;

/// Record a manual trigger for `id` and spawn the same command the crontab would run.
///
/// Refuses to launch (with a distinct [`AppError::Locked`] message) when the routine is
/// user-disabled (`enabled: false`) or in power-saving mode — `enabled` and `power_saving` are
/// independent signals, checked in that order so the response names whichever one is actually
/// responsible.
#[cfg(test)]
pub fn svc_trigger(store: &RoutineStore, id: &str) -> Result<Routine, AppError> {
    svc_trigger_with_system_power_saving_override(store, id, false)
}

/// Record a manual trigger, optionally bypassing host-level power saving for this one run.
pub fn svc_trigger_with_system_power_saving_override(
    store: &RoutineStore,
    id: &str,
    override_system_power_saving: bool,
) -> Result<Routine, AppError> {
    if crate::global_lock::is_globally_locked() {
        return Err(AppError::Locked("routines are globally locked".into()));
    }
    let mut lock = store.lock_recover();
    let routine = lock.get_mut(id).ok_or(AppError::NotFound)?;
    if !routine.enabled {
        return Err(AppError::Locked("routine is disabled".into()));
    }
    if let Some(reason) = power_saving_block_reason(
        routine,
        crate::system_power::is_system_power_saving_active(),
        override_system_power_saving,
    ) {
        return Err(AppError::Locked(reason.into()));
    }
    let ts = now_secs();
    routine.last_manual_trigger_at = Some(ts);
    let routine = routine.clone();
    drop(lock);
    write_routine(&routine).map_err(|_| AppError::Internal)?;
    append_manual_trigger_log(&crate::routine_storage::routine_rel_dir(&routine), ts);
    spawn_routine_command(&routine, TriggerSource::Manual);
    Ok(routine)
}

/// Run a routine on its schedule: spawn the command the crontab line invokes, without recording a
/// *manual* trigger.
///
/// This is the daemon-side endpoint that the generated crontab line drives
/// (`moadim schedule trigger <id>`). Unlike [`svc_trigger`] it leaves `last_manual_trigger_at`
/// untouched — the spawned command appends the timestamp to the routine's `scheduled.log` itself,
/// which the daemon reads back on the next load. Keeping the two paths distinct preserves the
/// manual-vs-scheduled distinction the timestamps exist to capture.
///
/// A routine snoozed via [`svc_snooze`] (`snoozed_until` in the future, or `skip_runs` above zero)
/// is skipped here instead of spawned: `snoozed_until` clears itself once elapsed (that fire then
/// runs), `skip_runs` decrements once per skipped fire and clears at zero. [`svc_trigger`] (manual)
/// ignores both fields entirely, by design.
///
/// Also refuses to launch when the routine is user-disabled or in power-saving mode, same as
/// [`svc_trigger`] — checked first, ahead of snooze, since a disabled/power-saving routine should
/// never spawn regardless of its snooze state. In practice a disabled routine has no crontab line
/// (see `sync::routines::build_block`), so this branch is a defense-in-depth guard for direct calls
/// to this endpoint rather than the primary way disabled routines stay quiet.
pub fn svc_trigger_scheduled(store: &RoutineStore, id: &str) -> Result<Routine, AppError> {
    if crate::global_lock::is_globally_locked() {
        return Err(AppError::Locked("routines are globally locked".into()));
    }
    let mut lock = store.lock_recover();
    let routine = lock.get_mut(id).ok_or(AppError::NotFound)?;
    if !routine.enabled {
        return Err(AppError::Locked("routine is disabled".into()));
    }
    if let Some(reason) = power_saving_block_reason(
        routine,
        crate::system_power::is_system_power_saving_active(),
        false,
    ) {
        let ts = now_secs();
        let rel_dir = crate::routine_storage::routine_rel_dir(routine);
        drop(lock);
        append_skip_log(&rel_dir, ts, reason);
        return Err(AppError::Locked(reason.into()));
    }

    if let Some(until) = routine.snoozed_until {
        if now_secs() < until {
            return Err(AppError::Locked(format!("routine snoozed until {until}")));
        }
        routine.snoozed_until = None;
        let routine = routine.clone();
        drop(lock);
        write_routine(&routine).map_err(|_| AppError::Internal)?;
        spawn_routine_command(&routine, TriggerSource::Scheduled);
        return Ok(routine);
    }
    if let Some(runs) = routine.skip_runs {
        if runs > 0 {
            routine.skip_runs = (runs > 1).then_some(runs - 1);
            let routine = routine.clone();
            drop(lock);
            write_routine(&routine).map_err(|_| AppError::Internal)?;
            return Err(AppError::Locked(format!(
                "routine snoozed, skipping this scheduled run ({} more to skip)",
                routine.skip_runs.unwrap_or(0)
            )));
        }
    }

    let ts = now_secs();
    if !claim_current_minute(id, ts) {
        let reason = "routine already fired this minute; skipping duplicate scheduled trigger";
        let rel_dir = crate::routine_storage::routine_rel_dir(routine);
        drop(lock);
        append_skip_log(&rel_dir, ts, reason);
        return Err(AppError::Locked(reason.into()));
    }
    // Same-minute claim (#795): multiple scheduler jobs can target the same routine when a
    // multi-schedule routine has overlapping expressions. Claim the current minute in process while
    // holding the store mutex so a second scheduled-trigger request observes the claim and no-ops
    // instead of launching a duplicate workbench. The durable scheduled timestamp remains execution
    // history, and the spawned scheduled command still appends its entry to `scheduled.log`.
    routine.last_scheduled_trigger_at = Some(ts);
    let routine = routine.clone();
    drop(lock);
    spawn_routine_command(&routine, TriggerSource::Scheduled);
    Ok(routine)
}

/// Resolve the `sh` executable to invoke for a routine launch.
///
/// Honours the `MOADIM_SH_BIN` environment variable when set, falling back to the platform shell
/// (`sh`) otherwise. The override exists so tests can point the spawn at a shim instead of running
/// a real login shell.
///
/// In **test builds**, when no `MOADIM_SH_BIN` shim is configured this never falls back to the
/// real `sh`: it returns a path that cannot exist, so the spawn fails harmlessly instead of
/// launching a real agent process. This closes the same structural gap `crontab_bin()` in
/// `crate::sync` closes for crontab I/O (issue #175) — a test that forgets to
/// clear `PATH` or shim this binary still cannot execute a real command on the developer's
/// machine (issue #217). Tests that need a working spawn set `MOADIM_SH_BIN` to a shim.
pub(crate) fn sh_bin() -> String {
    if let Ok(bin) = std::env::var("MOADIM_SH_BIN") {
        return bin;
    }
    #[cfg(test)]
    let fallback = "/nonexistent/moadim-test-sh-guard".to_string();
    #[cfg(not(test))]
    let fallback = "sh".to_string();
    fallback
}

/// Return the power-saving reason that should block `routine`, if any.
pub(super) const fn power_saving_block_reason(
    routine: &Routine,
    system_active: bool,
    override_system_power_saving: bool,
) -> Option<&'static str> {
    if routine.power_saving {
        return Some("routine is in power-saving mode");
    }
    if system_active && !routine.power_saving_exempt && !override_system_power_saving {
        return Some("system power saving is active");
    }
    None
}

include!("svc_snooze.rs");
