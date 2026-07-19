//! Run-history listing: per-routine and fleet-wide, merging live workbenches with durable
//! `runs.log` records. Split out of `service_trigger.rs` to keep that file under the line-count
//! gate.

use crate::error::AppError;
use crate::paths::workbenches_dir;
use crate::utils::lock::LockRecover;
use crate::utils::time::format_local;

use crate::routines::cleanup::{parse_workbench_name, run_session_alive};
use crate::routines::command::slugify;
use crate::routines::model::{FleetRunSummary, RoutineStore, RunStatus, RunSummary};
use crate::routines::run_history::{read_exit_code, read_persisted_runs};

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
            started_at_local: format_local(persisted.started_at),
            finished_at: Some(persisted.finished_at),
            finished_at_local: Some(format_local(persisted.finished_at)),
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
/// without a workbench migration failure — see `migrate_workbenches`) is skipped: there is no
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
                started_at_local: run.started_at_local,
                finished_at: run.finished_at,
                finished_at_local: run.finished_at_local,
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
                started_at_local: format_local(persisted.started_at),
                finished_at: Some(persisted.finished_at),
                finished_at_local: Some(format_local(persisted.finished_at)),
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
/// `effective_ttl_secs` is the owning routine's `Routine::effective_ttl_secs`, used to compute
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
        started_at_local: format_local(started_at),
        finished_at,
        finished_at_local: finished_at.map(format_local),
        status,
        exit_code,
        retention_expires_at,
    }
}
