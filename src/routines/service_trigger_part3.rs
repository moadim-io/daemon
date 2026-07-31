
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
