//! `svc_update`, split out of `service.rs` to stay under the repo's 500-line-per-file cap.

use super::{
    map_write_routine_err, max_runtime_ceiling_secs, migrate_workbenches, normalize_model,
    normalize_schedule, now_secs, reject_blank, reject_over_ceiling, reject_zero_secs,
    remove_routine_dir, slugify, ttl_ceiling_secs, validate_agent, validate_cron, validate_goal,
    validate_machines, validate_prompt, validate_repositories, validate_tags, validate_title,
    write_routine, AppError, LockRecover, RoutineResponse, RoutineStore, UpdateRoutineRequest,
};

/// Apply non-`None` fields from `req` to the routine identified by `id`.
pub fn svc_update(
    store: &RoutineStore,
    id: &str,
    req: UpdateRoutineRequest,
) -> Result<RoutineResponse, AppError> {
    if let Some(ref sched) = req.schedule {
        validate_cron(sched)?;
    }
    if let Some(ref title) = req.title {
        reject_blank("title", title)?;
        validate_title(title)?;
    }
    if let Some(ref prompt) = req.prompt {
        validate_prompt(prompt)?;
    }
    if let Some(ref agent) = req.agent {
        validate_agent(agent)?;
    }
    reject_zero_secs("ttl_secs", req.ttl_secs)?;
    reject_zero_secs("max_runtime_secs", req.max_runtime_secs)?;
    let repositories = match req.repositories {
        Some(ref repos) => Some(validate_repositories(repos)?),
        None => None,
    };
    let tags = match req.tags {
        Some(ref tags) => Some(validate_tags(tags)?),
        None => None,
    };
    // `Some(None)` clears the goal (empty string sent), `Some(Some(_))` sets it, `None` keeps it.
    let goal = match req.goal {
        Some(ref goal) => Some(validate_goal(Some(goal))?),
        None => None,
    };
    let machines = match req.machines {
        Some(ref machines) => Some(validate_machines(machines)?),
        None => None,
    };
    let mut lock = store.lock_recover();
    let old_slug = slugify(&lock.get(id).ok_or(AppError::NotFound)?.title);
    // Check slug conflict before mutating.
    if let Some(ref new_title) = req.title {
        let new_slug = slugify(new_title);
        if new_slug != old_slug
            && lock
                .values()
                .any(|routine| routine.id != id && slugify(&routine.title) == new_slug)
        {
            return Err(AppError::Conflict(format!(
                "a routine with the name \"{new_slug}\" already exists"
            )));
        }
    }
    // Reject ttl/max-runtime above the cron-derived ceiling for the *effective* schedule (the new
    // one if supplied, else the routine's current schedule) — before any mutation, so a rejected
    // update leaves the in-memory store untouched (#468).
    let effective_schedule = match req.schedule.as_deref() {
        Some(schedule) => normalize_schedule(schedule),
        None => lock
            .get(id)
            .expect("id existence checked above, and the lock has been held continuously since")
            .schedule
            .clone(),
    };
    reject_over_ceiling(
        "ttl_secs",
        req.ttl_secs,
        ttl_ceiling_secs(&effective_schedule),
    )?;
    reject_over_ceiling(
        "max_runtime_secs",
        req.max_runtime_secs,
        max_runtime_ceiling_secs(&effective_schedule),
    )?;
    let routine = lock
        .get_mut(id)
        .expect("id existence checked above, and the lock has been held continuously since");
    if let Some(schedule) = req.schedule {
        routine.schedule = normalize_schedule(&schedule);
    }
    if let Some(title) = req.title {
        // Trim on rename for the same reason as `svc_create` above.
        routine.title = title.trim().to_string();
    }
    if let Some(agent) = req.agent {
        routine.agent = agent;
    }
    if let Some(model) = req.model {
        routine.model = normalize_model(Some(model));
    }
    if let Some(prompt) = req.prompt {
        routine.prompt = prompt;
    }
    if let Some(goal) = goal {
        routine.goal = goal;
    }
    if let Some(repositories) = repositories {
        routine.repositories = repositories;
    }
    if let Some(auto_pull) = req.auto_pull {
        routine.auto_pull = auto_pull;
    }
    if let Some(machines) = machines {
        routine.machines = machines;
    }
    if let Some(enabled) = req.enabled {
        routine.enabled = enabled;
    }
    if let Some(ttl) = req.ttl_secs {
        routine.ttl_secs = Some(ttl);
    }
    if let Some(max_runtime) = req.max_runtime_secs {
        routine.max_runtime_secs = Some(max_runtime);
    }
    if let Some(tags) = tags {
        routine.tags = tags;
    }
    routine.updated_at = now_secs();
    let routine = routine.clone();
    drop(lock);
    let new_slug = slugify(&routine.title);
    write_routine(&routine).map_err(|err| map_write_routine_err(&err))?;
    if new_slug != old_slug {
        migrate_workbenches(&old_slug, &new_slug);
        remove_routine_dir(&old_slug).map_err(|_| AppError::Internal)?;
    }
    if let Err(err) = crate::sync::routines::sync_routines_to_crontab(store) {
        log::warn!("crontab sync after routine update failed: {err}");
    }
    Ok(RoutineResponse::from_routine(routine))
}
