//! Store-mutating service functions: list, get, create, update, delete, trigger, and logs.

use uuid::Uuid;

use crate::cron_jobs::{normalize_schedule, validate_cron};
use crate::error::AppError;
use crate::paths::workbenches_dir;
use crate::routine_storage::{remove_routine_dir, write_routine};
use crate::utils::time::now_secs;

use super::agents::load_agent_command;
use super::cleanup::{cleanup_expired_workbenches, parse_workbench_name};
use super::command::{build_routine_command, slugify};
use super::model::{
    CleanupResponse, CreateRoutineRequest, Routine, RoutineListQuery, RoutineResponse, RoutineSort,
    RoutineStore, SortOrder, UpdateRoutineRequest,
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
/// reproduces the previous behaviour. The `repository` filter keeps routines
/// referencing a matching repository URL; `sort`/`order` control ordering.
pub fn svc_list(store: &RoutineStore, query: &RoutineListQuery) -> Vec<RoutineResponse> {
    let lock = store.lock().unwrap();
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

    // Sort ascending by the requested field, then flip for descending order.
    match query.sort {
        RoutineSort::Created => routines.sort_by_key(|routine| routine.created_at),
        RoutineSort::Updated => routines.sort_by_key(|routine| routine.updated_at),
        RoutineSort::Title => routines.sort_by_key(|routine| routine.title.to_lowercase()),
        RoutineSort::Repository => routines.sort_by_key(repo_sort_key),
    }
    if query.order == SortOrder::Desc {
        routines.reverse();
    }

    routines
        .into_iter()
        .map(RoutineResponse::from_routine)
        .collect()
}

/// Look up a routine by `id`, returning `NotFound` if it does not exist.
pub fn svc_get(store: &RoutineStore, id: &str) -> Result<RoutineResponse, AppError> {
    let routine = store
        .lock()
        .unwrap()
        .get(id)
        .cloned()
        .ok_or(AppError::NotFound)?;
    Ok(RoutineResponse::from_routine(routine))
}

/// Validate `req`, assign a UUID, persist (routine.toml + prompt.md), and sync the crontab.
pub fn svc_create(
    store: &RoutineStore,
    req: CreateRoutineRequest,
) -> Result<RoutineResponse, AppError> {
    validate_cron(&req.schedule)?;
    let slug = slugify(&req.title);
    {
        let lock = store.lock().unwrap();
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
        title: req.title,
        agent: req.agent,
        prompt: req.prompt,
        repositories: req.repositories,
        enabled: req.enabled,
        source: "managed".to_string(),
        created_at: now,
        updated_at: now,
        last_triggered_at: None,
        ttl_secs: req.ttl_secs,
    };
    write_routine(&routine).map_err(|_| AppError::Internal)?;
    store
        .lock()
        .unwrap()
        .insert(routine.id.clone(), routine.clone());
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
    let mut lock = store.lock().unwrap();
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
    let routine = lock.get_mut(id).unwrap();
    if let Some(schedule) = req.schedule {
        routine.schedule = normalize_schedule(&schedule);
    }
    if let Some(title) = req.title {
        routine.title = title;
    }
    if let Some(agent) = req.agent {
        routine.agent = agent;
    }
    if let Some(prompt) = req.prompt {
        routine.prompt = prompt;
    }
    if let Some(repositories) = req.repositories {
        routine.repositories = repositories;
    }
    if let Some(enabled) = req.enabled {
        routine.enabled = enabled;
    }
    if let Some(ttl) = req.ttl_secs {
        routine.ttl_secs = Some(ttl);
    }
    routine.updated_at = now_secs();
    let routine = routine.clone();
    drop(lock);
    let new_slug = slugify(&routine.title);
    write_routine(&routine).map_err(|_| AppError::Internal)?;
    if new_slug != old_slug {
        remove_routine_dir(&old_slug).map_err(|_| AppError::Internal)?;
    }
    if let Err(err) = crate::sync::routines::sync_routines_to_crontab(store) {
        log::warn!("crontab sync after routine update failed: {err}");
    }
    Ok(RoutineResponse::from_routine(routine))
}

/// Remove the routine with `id` from the store and disk, then sync the crontab.
pub fn svc_delete(store: &RoutineStore, id: &str) -> Result<RoutineResponse, AppError> {
    let routine = store.lock().unwrap().remove(id).ok_or(AppError::NotFound)?;
    remove_routine_dir(&slugify(&routine.title)).map_err(|_| AppError::Internal)?;
    if let Err(err) = crate::sync::routines::sync_routines_to_crontab(store) {
        log::warn!("crontab sync after routine delete failed: {err}");
    }
    Ok(RoutineResponse::from_routine(routine))
}

/// Record a manual trigger for `id` and spawn the same command the crontab would run.
pub fn svc_trigger(store: &RoutineStore, id: &str) -> Result<Routine, AppError> {
    let mut lock = store.lock().unwrap();
    let routine = lock.get_mut(id).ok_or(AppError::NotFound)?;
    routine.last_triggered_at = Some(now_secs());
    let routine = routine.clone();
    drop(lock);
    write_routine(&routine).map_err(|_| AppError::Internal)?;
    match load_agent_command(&routine.agent) {
        Some(agent) => {
            let cmd = build_routine_command(&routine, &agent);
            #[cfg(not(windows))]
            {
                if let Err(err) = std::process::Command::new("sh").arg("-c").arg(&cmd).spawn() {
                    log::warn!("trigger: failed to spawn routine command: {err}");
                }
            }
            #[cfg(windows)]
            {
                crate::platform::spawn_routine_now(&cmd);
            }
        }
        None => log::warn!(
            "trigger: agent config not found for routine {:?} (agent {:?})",
            routine.id,
            routine.agent
        ),
    }
    Ok(routine)
}

/// Reap finished, expired run workbenches immediately, returning how many were removed.
///
/// Runs the same sweep as the hourly background task ([`cleanup_expired_workbenches`]) but on
/// demand, so callers need not wait for the next tick. Still-running sessions are never touched.
pub fn svc_cleanup(store: &RoutineStore) -> CleanupResponse {
    CleanupResponse {
        removed: cleanup_expired_workbenches(store),
    }
}

/// Return the contents of the newest workbench `agent.log` for routine `id`.
pub fn svc_logs(store: &RoutineStore, id: &str) -> Result<String, AppError> {
    let routine = store
        .lock()
        .unwrap()
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
        return Ok(String::new());
    };
    let log_path = workbenches_dir().join(dir).join("agent.log");
    if !log_path.exists() {
        return Ok(String::new());
    }
    std::fs::read_to_string(&log_path).map_err(|_| AppError::Internal)
}

#[cfg(test)]
#[path = "service_tests.rs"]
mod service_tests;
