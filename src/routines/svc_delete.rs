#![allow(
    clippy::wildcard_imports,
    clippy::too_many_arguments,
    clippy::needless_pass_by_value,
    dead_code,
    reason = "split-out module preserves existing code while keeping files under the linecheck limit"
)]
use super::*;

/// Remove the routine with `id` from the store and disk, then sync the crontab.
///
/// Also force-kills any in-flight workbench session(s) for this routine's slug, so a deleted
/// routine's agent doesn't keep running unsupervised until the next TTL sweep (#333).
///
/// When `id` is a built-in default, records a tombstone (#265) so
/// [`crate::routines::defaults::ensure_default_routines`] does not resurrect it, enabled, on the next
/// startup — deleting a default is a deliberate "I never want this" gesture, not a no-op.
pub fn svc_delete(store: &RoutineStore, id: &str) -> Result<RoutineResponse, AppError> {
    let routine = store.lock_recover().remove(id).ok_or(AppError::NotFound)?;
    let slug = routine_slug(&routine);
    let rel_dir = routine_rel_dir(&routine);
    let killed = kill_sessions_for_deleted_routine(&slug);
    if killed > 0 {
        log::warn!(
            "routine delete: killed {killed} in-flight session(s) for deleted routine {slug:?}"
        );
    }
    remove_routine_dir(&rel_dir).map_err(|_| AppError::Internal)?;
    if is_default_slug(&slug) {
        record_removed_default(&slug);
    }
    if let Err(err) = crate::sync::routines::sync_routines_to_crontab(store) {
        log::warn!("crontab sync after routine delete failed: {err}");
    }
    Ok(RoutineResponse::from_routine(routine))
}
