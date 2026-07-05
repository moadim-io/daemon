//! Store-mutating service functions: list, get, create, update, delete, trigger, and logs.

use crate::utils::lock::LockRecover;
use uuid::Uuid;

use crate::error::AppError;
use crate::routine_storage::{remove_routine_dir, write_routine};
use crate::utils::cron::{normalize_schedule, validate_cron};
use crate::utils::time::now_secs;

use super::cleanup::{
    kill_sessions_for_deleted_routine, max_runtime_ceiling_secs, ttl_ceiling_secs,
};
use super::command::slugify;
use super::defaults::{clear_removed_default, is_default_slug, record_removed_default};
#[cfg(test)]
use super::model::Repository;
use super::model::{
    CreateRoutineRequest, Routine, RoutineListQuery, RoutineResponse, RoutineSort, RoutineStore,
    SortOrder, UpdateRoutineRequest,
};

#[path = "service_validate.rs"]
mod service_validate;
#[cfg(test)]
use service_validate::MAX_TITLE_LEN;
use service_validate::{
    normalize_model, reject_blank, reject_over_ceiling, reject_zero_secs, validate_agent,
    validate_goal, validate_machines, validate_prompt, validate_repositories, validate_tags,
    validate_title,
};

/// Sort key placing routines with a repository before those without, then by
/// the primary (first) repository URL alphabetically (case-insensitive).
fn repo_sort_key(routine: &Routine) -> (bool, String) {
    match routine.repositories.first() {
        Some(repo) => (false, repo.repository.to_lowercase()),
        None => (true, String::new()),
    }
}

/// Return the routines matching `query`, filtered and sorted as requested.
///
/// The default query (no repository filter, sort by creation time ascending)
/// reproduces the previous behaviour, except each routine's `prompt` is omitted
/// unless `include_prompts` is `true`. The `repository` filter keeps routines
/// referencing a matching repository URL; `sort`/`order` control ordering.
pub fn svc_list(store: &RoutineStore, query: &RoutineListQuery) -> Vec<RoutineResponse> {
    let lock = store.lock_recover();
    let mut routines: Vec<Routine> = lock.values().cloned().collect();
    drop(lock);

    // Filter: keep routines with a repository URL containing the substring (case-insensitive).
    if let Some(needle) = query
        .repository
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let needle = needle.to_lowercase();
        routines.retain(|routine| {
            routine
                .repositories
                .iter()
                .any(|repo| repo.repository.to_lowercase().contains(&needle))
        });
    }

    // Filter: keep only routines that target the current machine.
    if query.local_only.unwrap_or(false) {
        let me = crate::machine::current_machine();
        routines.retain(|routine| crate::machine::targets(&routine.machines, &me));
    }

    // Sort by the requested field. The routines come off a `HashMap`, whose
    // iteration order is unspecified, so equal sort keys would otherwise list
    // in an arbitrary, run-to-run order. Break ties on the stable routine id to
    // make the listing deterministic, and reverse the whole comparison (not the
    // sorted vector) for descending order so the id tiebreak stays consistent.
    let desc = query.order == SortOrder::Desc;
    routines.sort_by(|left, right| {
        let primary = match query.sort {
            RoutineSort::Created => left.created_at.cmp(&right.created_at),
            RoutineSort::Updated => left.updated_at.cmp(&right.updated_at),
            RoutineSort::Title => left.title.to_lowercase().cmp(&right.title.to_lowercase()),
            RoutineSort::Repository => repo_sort_key(left).cmp(&repo_sort_key(right)),
        };
        let ord = primary.then_with(|| left.id.cmp(&right.id));
        if desc {
            ord.reverse()
        } else {
            ord
        }
    });

    // Omit prompts by default: they are the largest field and rarely needed in a listing.
    // Blanking triggers `skip_serializing_if` on `Routine::prompt`, dropping it from the JSON.
    let include_prompts = query.include_prompts.unwrap_or(false);

    routines
        .into_iter()
        .map(|mut routine| {
            if !include_prompts {
                routine.prompt.clear();
            }
            RoutineResponse::from_routine(routine)
        })
        .collect()
}

/// Look up a routine by `id`, returning `NotFound` if it does not exist.
pub fn svc_get(store: &RoutineStore, id: &str) -> Result<RoutineResponse, AppError> {
    let routine = store
        .lock_recover()
        .get(id)
        .cloned()
        .ok_or(AppError::NotFound)?;
    Ok(RoutineResponse::from_routine(routine))
}

/// Validate `req`, assign a UUID, persist (routine.toml + prompts/ sidecars), and sync the crontab.
pub fn svc_create(
    store: &RoutineStore,
    req: CreateRoutineRequest,
) -> Result<RoutineResponse, AppError> {
    validate_cron(&req.schedule)?;
    reject_blank("title", &req.title)?;
    validate_prompt(&req.prompt)?;
    reject_zero_secs("ttl_secs", req.ttl_secs)?;
    reject_zero_secs("max_runtime_secs", req.max_runtime_secs)?;
    let ceiling_schedule = normalize_schedule(&req.schedule);
    reject_over_ceiling(
        "ttl_secs",
        req.ttl_secs,
        ttl_ceiling_secs(&ceiling_schedule),
    )?;
    reject_over_ceiling(
        "max_runtime_secs",
        req.max_runtime_secs,
        max_runtime_ceiling_secs(&ceiling_schedule),
    )?;
    validate_title(&req.title)?;
    validate_agent(&req.agent)?;
    let repositories = validate_repositories(&req.repositories)?;
    let tags = validate_tags(&req.tags)?;
    let goal = validate_goal(req.goal.as_deref())?;
    let machines = validate_machines(&req.machines)?;
    let slug = slugify(&req.title);
    {
        let lock = store.lock_recover();
        if lock.values().any(|routine| slugify(&routine.title) == slug) {
            return Err(AppError::Conflict(format!(
                "a routine with the name \"{slug}\" already exists"
            )));
        }
    }
    let now = now_secs();
    let routine = Routine {
        id: Uuid::new_v4().to_string(),
        schedule: normalize_schedule(&req.schedule),
        // Trim before persisting so a padded title (`"  Deploy  "`) is not rendered
        // verbatim into the workbench `CLAUDE.md` disclosure, the iCal `SUMMARY`, and
        // the UI rows. Mirrors `validate_repositories`, which already normalizes the
        // repository fields, and `validate_title`, which length-checks the trimmed value.
        title: req.title.trim().to_string(),
        agent: req.agent,
        model: normalize_model(req.model),
        prompt: req.prompt,
        goal,
        repositories,
        machines,
        enabled: req.enabled,
        source: "managed".to_string(),
        created_at: now,
        updated_at: now,
        last_manual_trigger_at: None,
        last_scheduled_trigger_at: None,
        snoozed_until: None,
        skip_runs: None,
        // Power saving is system-driven, never settable via create/update — see
        // `svc_set_power_saving`.
        power_saving: false,
        ttl_secs: req.ttl_secs,
        max_runtime_secs: req.max_runtime_secs,
        tags,
    };
    write_routine(&routine).map_err(|_| AppError::Internal)?;
    store
        .lock_recover()
        .insert(routine.id.clone(), routine.clone());
    // A user re-creating a routine under a tombstoned default's title is a deliberate "bring it
    // back" signal (#265) — clear the tombstone so a future startup can seed the default again.
    if is_default_slug(&slug) {
        clear_removed_default(&slug);
    }
    if let Err(err) = crate::sync::routines::sync_routines_to_crontab(store) {
        log::warn!("crontab sync after routine create failed: {err}");
    }
    Ok(RoutineResponse::from_routine(routine))
}

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
    write_routine(&routine).map_err(|_| AppError::Internal)?;
    if new_slug != old_slug {
        migrate_workbenches(&old_slug, &new_slug);
        remove_routine_dir(&old_slug).map_err(|_| AppError::Internal)?;
    }
    if let Err(err) = crate::sync::routines::sync_routines_to_crontab(store) {
        log::warn!("crontab sync after routine update failed: {err}");
    }
    Ok(RoutineResponse::from_routine(routine))
}

/// Rename `old_name` to `new_name` in every routine's `machines` list, persist each changed
/// routine to disk, and sync the crontab so the new machine identity takes effect immediately.
///
/// Called automatically by `put_machine` so that renaming this daemon's machine identity also
/// updates all the routines that targeted it by the old name.
pub fn svc_rename_machine(store: &RoutineStore, old_name: &str, new_name: &str) {
    if old_name == new_name {
        return;
    }
    let now = now_secs();
    let updated: Vec<_> = {
        let mut lock = store.lock_recover();
        lock.values_mut()
            .filter(|routine| routine.machines.iter().any(|machine| machine == old_name))
            .map(|routine| {
                for machine in &mut routine.machines {
                    if machine == old_name {
                        *machine = new_name.to_string();
                    }
                }
                routine.updated_at = now;
                routine.clone()
            })
            .collect()
    };
    for routine in &updated {
        if let Err(err) = write_routine(routine) {
            log::warn!(
                "failed to persist machine rename for routine {}: {err}",
                routine.id
            );
        }
    }
    if !updated.is_empty() {
        if let Err(err) = crate::sync::routines::sync_routines_to_crontab(store) {
            log::warn!("crontab sync after machine rename failed: {err}");
        }
    }
}

/// Remove the routine with `id` from the store and disk, then sync the crontab.
///
/// Also force-kills any in-flight workbench session(s) for this routine's slug, so a deleted
/// routine's agent doesn't keep running unsupervised until the next TTL sweep (#333).
///
/// When `id` is a built-in default, records a tombstone (#265) so
/// [`super::defaults::ensure_default_routines`] does not resurrect it, enabled, on the next
/// startup — deleting a default is a deliberate "I never want this" gesture, not a no-op.
pub fn svc_delete(store: &RoutineStore, id: &str) -> Result<RoutineResponse, AppError> {
    let routine = store.lock_recover().remove(id).ok_or(AppError::NotFound)?;
    let slug = slugify(&routine.title);
    let killed = kill_sessions_for_deleted_routine(&slug);
    if killed > 0 {
        log::warn!(
            "routine delete: killed {killed} in-flight session(s) for deleted routine {slug:?}"
        );
    }
    remove_routine_dir(&slug).map_err(|_| AppError::Internal)?;
    if is_default_slug(&slug) {
        record_removed_default(&slug);
    }
    if let Err(err) = crate::sync::routines::sync_routines_to_crontab(store) {
        log::warn!("crontab sync after routine delete failed: {err}");
    }
    Ok(RoutineResponse::from_routine(routine))
}

#[path = "service_trigger.rs"]
mod service_trigger;
use service_trigger::migrate_workbenches;
#[cfg(test)]
pub(crate) use service_trigger::sh_bin;
#[cfg(test)]
pub(crate) use service_trigger::{read_log_tail, strip_ansi_noise, MAX_LOG_TAIL_BYTES};
pub use service_trigger::{
    svc_cleanup, svc_create_flag, svc_list_all_runs, svc_list_flags, svc_list_runs, svc_logs,
    svc_resolve_flag, svc_run_log, svc_set_power_saving, svc_snooze, svc_trigger,
    svc_trigger_scheduled,
};

#[cfg(test)]
#[path = "service_tests.rs"]
mod service_tests;

#[cfg(test)]
#[path = "service_sync_tests.rs"]
mod service_sync_tests;

#[cfg(test)]
#[path = "service_flag_tests.rs"]
mod service_flag_tests;

#[cfg(test)]
#[path = "service_model_tests.rs"]
mod service_model_tests;

#[cfg(test)]
#[path = "service_logs_tests.rs"]
mod service_logs_tests;

#[cfg(test)]
#[path = "service_runs_tests.rs"]
mod service_runs_tests;

#[cfg(test)]
#[path = "service_trigger_tests.rs"]
mod service_trigger_tests;

#[cfg(test)]
#[path = "service_power_saving_tests.rs"]
mod service_power_saving_tests;

#[cfg(test)]
#[path = "service_coverage_tests.rs"]
mod service_coverage_tests;

#[cfg(test)]
#[path = "service_slug_tests.rs"]
mod service_slug_tests;

#[cfg(test)]
#[path = "service_overlap_guard_tests.rs"]
mod service_overlap_guard_tests;

#[cfg(test)]
#[path = "service_update_not_found_tests.rs"]
mod service_update_not_found_tests;
