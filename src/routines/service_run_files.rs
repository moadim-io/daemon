//! Per-run file reads: a specific run's `agent.log` tail and agent-authored `summary.md`, plus a
//! side-effect-free preview of the composed prompt a run would receive.

use crate::error::AppError;
use crate::paths::workbenches_dir;
use crate::routines::cleanup::parse_workbench_name;
use crate::routines::command::{compose_prompt, slugify};
use crate::routines::model::RoutineStore;
use crate::utils::lock::LockRecover;

use super::service_log_tail::read_log_tail;

/// Return the contents of `filename` inside a specific run's workbench, by workbench directory
/// name.
///
/// `workbench` must be an existing, exact `{slug}-{ts}` directory belonging to routine `id` — this
/// guards both path traversal (a bare directory name, not an arbitrary path, is joined onto
/// `workbenches_dir()`) and cross-routine leakage, mirroring the exact-slug check in `svc_logs`.
/// Shared by [`svc_run_log`] (`agent.log`) and [`svc_run_summary`] (`summary.md`).
fn svc_run_file(
    store: &RoutineStore,
    id: &str,
    workbench: &str,
    filename: &str,
) -> Result<String, AppError> {
    let routine = store
        .lock_recover()
        .get(id)
        .cloned()
        .ok_or(AppError::NotFound)?;
    let slug = slugify(&routine.title);
    let Some((dir_slug, _)) = parse_workbench_name(workbench) else {
        return Err(AppError::NotFound);
    };
    if dir_slug != slug {
        return Err(AppError::NotFound);
    }
    let path = workbenches_dir().join(workbench).join(filename);
    if !path.exists() {
        return Ok(String::new());
    }
    read_log_tail(&path).map_err(|_| AppError::Internal)
}

/// Return the contents of a specific run's `agent.log`, by workbench directory name.
pub fn svc_run_log(store: &RoutineStore, id: &str, workbench: &str) -> Result<String, AppError> {
    svc_run_file(store, id, workbench, "agent.log")
}

/// Return the contents of a specific run's agent-authored `summary.md` (see the "Work log"
/// instruction in [`crate::routines::command::system_prompt_stmts`]), by workbench directory
/// name. Empty string when the agent hasn't written one (yet, or never did).
pub fn svc_run_summary(
    store: &RoutineStore,
    id: &str,
    workbench: &str,
) -> Result<String, AppError> {
    svc_run_file(store, id, workbench, "summary.md")
}

/// Compose the exact body an agent run would receive for routine `id`, without creating a
/// workbench, writing `prompt.md`, or launching an agent (issue #391). Mirrors `svc_get`'s lookup,
/// but returns the *derived* prompt body instead of the stored routine fields.
///
/// Includes the routine-origin disclosure because it is now part of [`compose_prompt`].
pub fn svc_get_prompt_preview(store: &RoutineStore, id: &str) -> Result<String, AppError> {
    let routine = store
        .lock_recover()
        .get(id)
        .cloned()
        .ok_or(AppError::NotFound)?;
    Ok(compose_prompt(&routine))
}

#[cfg(test)]
#[path = "service_prompt_preview_tests.rs"]
mod service_prompt_preview_tests;
