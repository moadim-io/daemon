//! Store-mutating service functions: list, get, create, update, delete, trigger, and logs.

use uuid::Uuid;

use crate::cron_jobs::{normalize_schedule, validate_cron};
use crate::error::AppError;
use crate::paths::workbenches_dir;
use crate::routine_storage::{remove_routine_dir, write_routine};
use crate::utils::time::now_secs;

use super::agents::load_agent_command;
use super::command::{build_routine_command, slugify};
use super::model::{
    CreateRoutineRequest, Routine, RoutineResponse, RoutineStore, UpdateRoutineRequest,
};

/// Return all routines sorted by creation time (oldest first).
pub fn svc_list(store: &RoutineStore) -> Vec<RoutineResponse> {
    let lock = store.lock().unwrap();
    let mut routines: Vec<Routine> = lock.values().cloned().collect();
    routines.sort_by_key(|r| r.created_at);
    drop(lock);
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
        if lock.values().any(|r| slugify(&r.title) == slug) {
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
    };
    write_routine(&routine).map_err(|_| AppError::Internal)?;
    store
        .lock()
        .unwrap()
        .insert(routine.id.clone(), routine.clone());
    if let Err(e) = crate::sync::routines::sync_routines_to_crontab(store) {
        log::warn!("crontab sync after routine create failed: {e}");
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
        if new_slug != old_slug && lock.values().any(|r| r.id != id && slugify(&r.title) == new_slug) {
            return Err(AppError::Conflict(format!(
                "a routine with the name \"{new_slug}\" already exists"
            )));
        }
    }
    let routine = lock.get_mut(id).unwrap();
    if let Some(s) = req.schedule {
        routine.schedule = normalize_schedule(&s);
    }
    if let Some(t) = req.title {
        routine.title = t;
    }
    if let Some(a) = req.agent {
        routine.agent = a;
    }
    if let Some(p) = req.prompt {
        routine.prompt = p;
    }
    if let Some(r) = req.repositories {
        routine.repositories = r;
    }
    if let Some(e) = req.enabled {
        routine.enabled = e;
    }
    routine.updated_at = now_secs();
    let routine = routine.clone();
    drop(lock);
    let new_slug = slugify(&routine.title);
    write_routine(&routine).map_err(|_| AppError::Internal)?;
    if new_slug != old_slug {
        remove_routine_dir(&old_slug).map_err(|_| AppError::Internal)?;
    }
    if let Err(e) = crate::sync::routines::sync_routines_to_crontab(store) {
        log::warn!("crontab sync after routine update failed: {e}");
    }
    Ok(RoutineResponse::from_routine(routine))
}

/// Remove the routine with `id` from the store and disk, then sync the crontab.
pub fn svc_delete(store: &RoutineStore, id: &str) -> Result<RoutineResponse, AppError> {
    let routine = store.lock().unwrap().remove(id).ok_or(AppError::NotFound)?;
    remove_routine_dir(&slugify(&routine.title)).map_err(|_| AppError::Internal)?;
    if let Err(e) = crate::sync::routines::sync_routines_to_crontab(store) {
        log::warn!("crontab sync after routine delete failed: {e}");
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
            if let Err(e) = std::process::Command::new("sh").arg("-c").arg(&cmd).spawn() {
                log::warn!("trigger: failed to spawn routine command: {e}");
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

/// Return the contents of the newest workbench `agent.log` for routine `id`.
pub fn svc_logs(store: &RoutineStore, id: &str) -> Result<String, AppError> {
    let routine = store
        .lock()
        .unwrap()
        .get(id)
        .cloned()
        .ok_or(AppError::NotFound)?;
    let prefix = format!("{}-", slugify(&routine.title));
    let mut newest: Option<String> = None;
    if let Ok(entries) = std::fs::read_dir(workbenches_dir()) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with(&prefix) && newest.as_ref().is_none_or(|n| name > *n) {
                newest = Some(name);
            }
        }
    }
    let Some(dir) = newest else {
        return Ok(String::new());
    };
    let log_path = workbenches_dir().join(dir).join("agent.log");
    if !log_path.exists() {
        return Ok(String::new());
    }
    std::fs::read_to_string(&log_path).map_err(|_| AppError::Internal)
}
