//! Manual/scheduled triggers, snooze, cleanup, logs, and flags for routines.

use crate::error::AppError;
use crate::paths::workbenches_dir;
use crate::routine_storage::{append_manual_trigger_log, write_routine};
use crate::utils::lock::LockRecover;
use crate::utils::time::now_secs;

use crate::routines::agents::load_agent_command;
use crate::routines::cleanup::{
    cleanup_expired_workbenches, parse_workbench_name, run_session_alive, tmux_session_prefix_alive,
};
use crate::routines::command::{
    build_routine_command, inline_prompt_overflow, slugify, tmux_session_prefix,
};
use crate::routines::model::{
    CleanupResponse, FleetRunSummary, Routine, RoutineStore, RunStatus, RunSummary,
};
use crate::routines::run_history::{read_exit_code, read_persisted_runs};

use super::service_log_tail::{read_log_tail_with_meta, LogWithMeta};

/// Record a manual trigger for `id` and spawn the same command the crontab would run.
///
/// Refuses to launch (with a distinct [`AppError::Locked`] message) when the routine is
/// user-disabled (`enabled: false`) or in power-saving mode — `enabled` and `power_saving` are
/// independent signals, checked in that order so the response names whichever one is actually
/// responsible.
pub fn svc_trigger(store: &RoutineStore, id: &str) -> Result<Routine, AppError> {
    if crate::global_lock::is_globally_locked() {
        return Err(AppError::Locked("routines are globally locked".into()));
    }
    let mut lock = store.lock_recover();
    let routine = lock.get_mut(id).ok_or(AppError::NotFound)?;
    if !routine.enabled {
        return Err(AppError::Locked("routine is disabled".into()));
    }
    if routine.power_saving {
        return Err(AppError::Locked("routine is in power-saving mode".into()));
    }
    let ts = now_secs();
    routine.last_manual_trigger_at = Some(ts);
    let routine = routine.clone();
    drop(lock);
    write_routine(&routine).map_err(|_| AppError::Internal)?;
    append_manual_trigger_log(&crate::routines::slugify(&routine.title), ts);
    spawn_routine_command(&routine);
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
    if routine.power_saving {
        return Err(AppError::Locked("routine is in power-saving mode".into()));
    }

    if let Some(until) = routine.snoozed_until {
        if now_secs() < until {
            return Err(AppError::Locked(format!("routine snoozed until {until}")));
        }
        routine.snoozed_until = None;
        let routine = routine.clone();
        drop(lock);
        write_routine(&routine).map_err(|_| AppError::Internal)?;
        spawn_routine_command(&routine);
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

    let routine = routine.clone();
    drop(lock);
    spawn_routine_command(&routine);
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

/// Set or clear a routine's snooze state, skipping its upcoming *scheduled* fires (see
/// [`svc_trigger_scheduled`]) without touching `enabled` or the crontab. Manual triggers
/// ([`svc_trigger`]) always ignore snooze.
///
/// `snoozed_until` and `skip_runs` are mutually exclusive: passing both `Some` is a
/// [`AppError::BadRequest`]. Passing both `None` clears an active snooze.
pub fn svc_snooze(
    store: &RoutineStore,
    id: &str,
    snoozed_until: Option<u64>,
    skip_runs: Option<u32>,
) -> Result<Routine, AppError> {
    if snoozed_until.is_some() && skip_runs.is_some() {
        return Err(AppError::BadRequest(
            "snoozed_until and skip_runs are mutually exclusive; set only one".into(),
        ));
    }
    let mut lock = store.lock_recover();
    let routine = lock.get_mut(id).ok_or(AppError::NotFound)?;
    routine.snoozed_until = snoozed_until;
    routine.skip_runs = skip_runs;
    let routine = routine.clone();
    drop(lock);
    write_routine(&routine).map_err(|_| AppError::Internal)?;
    Ok(routine)
}

/// Set or clear a routine's power-saving state, without touching `enabled` or the crontab.
///
/// System/policy-owned, orthogonal to the user-owned `enabled` toggle (see
/// [`Routine::power_saving`]): both [`svc_trigger`] and [`svc_trigger_scheduled`] refuse to launch
/// while it is active, but the routine keeps its crontab line and its `enabled` value is untouched,
/// so it resumes firing on its own once power saving is cleared.
pub fn svc_set_power_saving(
    store: &RoutineStore,
    id: &str,
    active: bool,
) -> Result<Routine, AppError> {
    let mut lock = store.lock_recover();
    let routine = lock.get_mut(id).ok_or(AppError::NotFound)?;
    routine.power_saving = active;
    let routine = routine.clone();
    drop(lock);
    write_routine(&routine).map_err(|_| AppError::Internal)?;
    Ok(routine)
}

/// Spawn the launch command for `routine` under a login shell, logging (rather than failing) when
/// the agent config cannot be loaded, the composed prompt won't fit in an inlined `{prompt}`
/// argument, a previous fire of this routine is still running, or the process cannot be spawned.
///
/// `sh -lc` sources the user's `~/.profile`, so the agent inherits their environment (`GH_TOKEN`,
/// API keys, …) regardless of the minimal environment the daemon (or cron) runs under. Shared by the
/// manual ([`svc_trigger`]) and scheduled ([`svc_trigger_scheduled`]) paths.
fn spawn_routine_command(routine: &Routine) {
    match load_agent_command(&routine.agent) {
        Ok(agent) => {
            // Guard against the silent `execve(E2BIG)` no-op an oversized `{prompt}` argument
            // causes inside the detached tmux session (#443): the OS-level failure never
            // surfaces anywhere, so catch it here instead and skip the launch with a visible
            // warning, the same non-fatal shape as the agent-load-failure arm below.
            if let Some(len) = inline_prompt_overflow(routine, &agent) {
                log::warn!(
                    "trigger: composed prompt for routine {:?} is {len} bytes, over the \
                     inline-argument limit for agent {:?}; skipping launch (would fail silently \
                     inside tmux otherwise) — switch the agent's args to {{prompt_file}} or \
                     shorten the routine's prompt/open flags",
                    routine.id,
                    routine.agent,
                );
                return;
            }
            // Overlap guard (#514): a routine has no built-in mutual exclusion between fires, so a
            // run outliving its schedule interval would otherwise pile up concurrent agent sessions
            // all acting on the same target — duplicate PRs/issues, racing pushes. Every fire's tmux
            // session name shares the same `moadim-{slug}-` prefix (see `build_routine_command`); if
            // any of them is still alive, skip this fire instead of launching a second one.
            let session_prefix = tmux_session_prefix(&slugify(&routine.title));
            if tmux_session_prefix_alive(&session_prefix) {
                log::warn!(
                    "trigger: routine {:?} skipped — a previous run (tmux session prefix {:?}) is \
                     still active (overlap guard)",
                    routine.id,
                    session_prefix,
                );
                return;
            }
            let cmd = build_routine_command(routine, &agent);
            // `-lc` (login shell) mirrors the crontab invocation (`/bin/sh -l <run.sh>`), so a
            // manual trigger sources the user's `~/.profile` and the agent gets the same
            // environment whether fired by cron or on demand.
            let mut command = std::process::Command::new(sh_bin());
            command.arg("-lc").arg(&cmd);
            // Reap the child in the background so the short-lived launcher shell does not
            // linger as a zombie for the daemon's lifetime (the trigger stays non-blocking).
            crate::utils::process::spawn_and_reap(command, "routine command");
        }
        Err(err) => log::warn!(
            "trigger: cannot load agent {:?} ({}) for routine {:?}",
            routine.agent,
            err,
            routine.id
        ),
    }
}

/// Reap finished, expired run workbenches immediately, returning how many were removed and the
/// bytes freed.
///
/// Runs the same sweep as the hourly background task ([`cleanup_expired_workbenches`]) but on
/// demand, so callers need not wait for the next tick. Still-running sessions are never touched.
pub fn svc_cleanup(store: &RoutineStore) -> CleanupResponse {
    let stats = cleanup_expired_workbenches(store);
    CleanupResponse {
        removed: stats.removed,
        freed_bytes: stats.freed_bytes,
    }
}

/// Rename every existing workbench directory from `old_slug` to `new_slug`, preserving each run's
/// trigger timestamp (`{old_slug}-{ts}` -> `{new_slug}-{ts}`).
///
/// Called from `svc_update` when a routine's title (and thus slug) changes. Workbenches are keyed
/// by slug, not the routine's stable UUID, so without this migration a rename would strand every
/// prior run under the old slug: [`svc_logs`] (which looks up by *current* slug) would find nothing,
/// and an in-flight run would fall through to the cleanup watchdog's orphan defaults instead of the
/// routine's own `ttl_secs`/`max_runtime_secs` (#267). A failed rename is logged and skipped rather
/// than failing the update itself — this is best-effort history preservation, not a correctness
/// requirement of the rename.
pub(super) fn migrate_workbenches(old_slug: &str, new_slug: &str) {
    let Ok(entries) = std::fs::read_dir(workbenches_dir()) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let Some((dir_slug, ts)) = parse_workbench_name(&name) else {
            continue;
        };
        if dir_slug != old_slug {
            continue;
        }
        let from = workbenches_dir().join(&name);
        let to = workbenches_dir().join(format!("{new_slug}-{ts}"));
        if let Err(err) = std::fs::rename(&from, &to) {
            log::warn!("failed to migrate workbench {name} to {new_slug}-{ts}: {err}");
        }
    }
}

/// Return the contents of the newest workbench `agent.log` for routine `id`, plus whether that
/// content is a truncated window rather than the complete file (see [`LogWithMeta`]).
pub fn svc_logs(store: &RoutineStore, id: &str) -> Result<LogWithMeta, AppError> {
    let routine = store
        .lock_recover()
        .get(id)
        .cloned()
        .ok_or(AppError::NotFound)?;
    let slug = slugify(&routine.title);
    let mut newest: Option<(u64, String)> = None;
    if let Ok(entries) = std::fs::read_dir(workbenches_dir()) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            // Select only this routine's own workbenches by an *exact* slug match.
            // A bare `{slug}-` prefix would also match another routine whose slug
            // begins with this one (e.g. `logs` vs `logs-extra`), leaking that
            // routine's log. Reusing the canonical `{slug}-{ts}` parser also makes
            // "newest" a numeric timestamp comparison rather than a lexicographic
            // one over the whole directory name.
            if let Some((dir_slug, ts)) = parse_workbench_name(&name) {
                if dir_slug == slug && newest.as_ref().is_none_or(|(newest_ts, _)| ts > *newest_ts)
                {
                    newest = Some((ts, name));
                }
            }
        }
    }
    let Some((_, dir)) = newest else {
        return Ok(LogWithMeta::empty());
    };
    let log_path = workbenches_dir().join(dir).join("agent.log");
    if !log_path.exists() {
        return Ok(LogWithMeta::empty());
    }
    read_log_tail_with_meta(&log_path).map_err(|_| AppError::Internal)
}

/// List every run for routine `id`, newest first: live (not-yet-reaped) workbenches, whose status
/// derives from the tmux session's liveness and the `exit_code` file the launch command writes on
/// completion (see [`crate::routines::command::build_routine_command`]), merged with durable
/// records from `runs.log` for runs whose workbench has since been TTL-reaped (see
/// [`crate::routines::run_history`]) — so this list is the routine's *full* history, not just what
/// current retention happens to keep.
pub fn svc_list_runs(store: &RoutineStore, id: &str) -> Result<Vec<RunSummary>, AppError> {
    let routine = store
        .lock_recover()
        .get(id)
        .cloned()
        .ok_or(AppError::NotFound)?;
    let slug = slugify(&routine.title);
    let mut runs = Vec::new();
    if let Ok(entries) = std::fs::read_dir(workbenches_dir()) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let Some((dir_slug, ts)) = parse_workbench_name(&name) else {
                continue;
            };
            if dir_slug != slug {
                continue;
            }
            runs.push(run_summary(&name, ts, Some(routine.effective_ttl_secs())));
        }
    }
    for persisted in read_persisted_runs(id) {
        runs.push(RunSummary {
            workbench: persisted.workbench,
            started_at: persisted.started_at,
            finished_at: Some(persisted.finished_at),
            status: persisted.status,
            exit_code: persisted.exit_code,
            // The workbench is already gone (that's why this run came from `runs.log` instead of
            // a live directory scan), so there is nothing left to count down to.
            retention_expires_at: None,
        });
    }
    runs.sort_by_key(|run| std::cmp::Reverse(run.started_at));
    Ok(runs)
}

/// Default cap on [`svc_list_all_runs`] results when the caller doesn't specify one.
pub const DEFAULT_FLEET_RUNS_LIMIT: usize = 20;

/// List the most recent runs across *every* routine, newest first, capped at `limit` (or
/// [`DEFAULT_FLEET_RUNS_LIMIT`] when `None`). Backs the overview "recent runs" panel with a single
/// workbench-directory scan, rather than one [`svc_list_runs`] call per routine. Merges in durable
/// `runs.log` records for TTL-reaped runs (see [`crate::routines::run_history`]) alongside live
/// workbenches.
///
/// A workbench whose slug matches no current routine (the routine was since deleted, or renamed
/// without a workbench migration failure — see [`migrate_workbenches`]) is skipped: there is no
/// routine to attribute it to.
pub fn svc_list_all_runs(store: &RoutineStore, limit: Option<usize>) -> Vec<FleetRunSummary> {
    let limit = limit.unwrap_or(DEFAULT_FLEET_RUNS_LIMIT);
    let routines: Vec<(String, String)> = store
        .lock_recover()
        .values()
        .map(|routine| (routine.id.clone(), routine.title.clone()))
        .collect();
    let by_slug: std::collections::HashMap<String, (String, String)> = routines
        .iter()
        .map(|(id, title)| (slugify(title), (id.clone(), title.clone())))
        .collect();
    let mut runs = Vec::new();
    if let Ok(entries) = std::fs::read_dir(workbenches_dir()) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let Some((dir_slug, ts)) = parse_workbench_name(&name) else {
                continue;
            };
            let Some((routine_id, routine_title)) = by_slug.get(dir_slug).cloned() else {
                continue;
            };
            let run = run_summary(&name, ts, None);
            runs.push(FleetRunSummary {
                routine_id,
                routine_title,
                workbench: run.workbench,
                started_at: run.started_at,
                finished_at: run.finished_at,
                status: run.status,
                exit_code: run.exit_code,
            });
        }
    }
    for (routine_id, routine_title) in &routines {
        for persisted in read_persisted_runs(routine_id) {
            runs.push(FleetRunSummary {
                routine_id: routine_id.clone(),
                routine_title: routine_title.clone(),
                workbench: persisted.workbench,
                started_at: persisted.started_at,
                finished_at: Some(persisted.finished_at),
                status: persisted.status,
                exit_code: persisted.exit_code,
            });
        }
    }
    runs.sort_by_key(|run| std::cmp::Reverse(run.started_at));
    runs.truncate(limit);
    runs
}

/// Derive a single [`RunSummary`] for workbench `dir` (named `{slug}-{started_at}`).
///
/// `effective_ttl_secs` is the owning routine's [`Routine::effective_ttl_secs`], used to compute
/// `retention_expires_at`; pass `None` when the caller (e.g. the fleet-wide
/// [`svc_list_all_runs`]) doesn't need that field.
fn run_summary(dir: &str, started_at: u64, effective_ttl_secs: Option<u64>) -> RunSummary {
    let path = workbenches_dir().join(dir);
    let exit_code = read_exit_code(&path);
    let finished_at = std::fs::metadata(path.join("exit_code"))
        .and_then(|meta| meta.modified())
        .ok()
        .and_then(|mtime| mtime.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|elapsed| elapsed.as_secs());
    let session = format!("moadim-{dir}");
    let status = match exit_code {
        Some(0) => RunStatus::Success,
        Some(_) => RunStatus::Failed,
        None if run_session_alive(&session) => RunStatus::Running,
        None => RunStatus::Unknown,
    };
    let retention_expires_at =
        finished_at.and_then(|finish| effective_ttl_secs.map(|ttl| finish + ttl));
    RunSummary {
        workbench: dir.to_string(),
        started_at,
        finished_at,
        status,
        exit_code,
        retention_expires_at,
    }
}
