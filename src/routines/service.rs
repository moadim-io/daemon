//! Store-mutating service functions: list, get, create, update, delete, trigger, and logs.

use uuid::Uuid;

use crate::cron_jobs::{normalize_schedule, validate_cron};
use crate::error::AppError;
use crate::paths::workbenches_dir;
use crate::routine_storage::{remove_routine_dir, write_routine};
use crate::utils::time::now_secs;

use super::agents::{available_agents, load_agent_command, AgentLoadError};
use super::cleanup::{cleanup_expired_workbenches, parse_workbench_name};
use super::command::{build_routine_command, slugify, TriggerSource};
use super::model::{
    CleanupResponse, CreateRoutineRequest, Repository, Routine, RoutineListQuery, RoutineResponse,
    RoutineSort, RoutineStore, SortOrder, UpdateRoutineRequest,
};

/// Reject a blank (empty or whitespace-only) required text field.
///
/// An empty `prompt` makes a routine fire forever with no task (#224); an empty
/// `title` yields an empty routine-origin disclosure name and a bare `"routine"`
/// slug (#226). Both are caught here before anything is persisted.
fn reject_blank(field: &str, value: &str) -> Result<(), AppError> {
    if value.trim().is_empty() {
        return Err(AppError::BadRequest(format!(
            "routine {field} must not be empty"
        )));
    }
    Ok(())
}

/// Reject a zero-second duration for an optional cap (`None` keeps the default).
///
/// `ttl_secs: 0` reaps a finished run's logs instantly and `max_runtime_secs: 0`
/// makes the watchdog kill the session the moment it starts (#233), so a supplied
/// value must be positive.
fn reject_zero_secs(field: &str, value: Option<u64>) -> Result<(), AppError> {
    if value == Some(0) {
        return Err(AppError::BadRequest(format!(
            "routine {field} must be greater than zero"
        )));
    }
    Ok(())
}

/// Sort key placing routines with a repository before those without, then by
/// the primary (first) repository URL alphabetically (case-insensitive).
fn repo_sort_key(routine: &Routine) -> (bool, String) {
    match routine.repositories.first() {
        Some(repo) => (false, repo.repository.to_lowercase()),
        None => (true, String::new()),
    }
}

/// Reject a referenced agent that is unknown or whose `<name>.toml` is present but unparseable.
///
/// Two failures are surfaced at edit time (REST 400 / MCP) instead of slipping through to fire time,
/// where they would only be logged and the routine silently skipped:
///
/// * An agent not present in the registry resolves to no command at fire time (#139). Mirrors the
///   `validate_cron` / slug-conflict guards.
/// * An agent whose config is present on disk but cannot be parsed (#189).
///
/// A *missing* config for a registered agent is intentionally allowed: the file may be created later,
/// and the missing-file case is handled (warned + skipped) downstream exactly as before.
fn validate_agent(agent: &str) -> Result<(), AppError> {
    let agents = available_agents();
    if !agents.iter().any(|known| known == agent) {
        return Err(AppError::BadRequest(format!(
            "unknown agent \"{agent}\"; valid agents: {}",
            agents.join(", ")
        )));
    }
    match load_agent_command(agent) {
        Ok(_) | Err(AgentLoadError::Missing) => Ok(()),
        Err(AgentLoadError::Parse(err)) => Err(AppError::BadRequest(format!(
            "agent {agent:?} has a malformed config: {err}"
        ))),
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

/// Upper bound on a routine title, in characters, to keep `CLAUDE.md`, crontab
/// comments, iCal `SUMMARY`s, and UI rows from rendering an unbounded string.
const MAX_TITLE_LEN: usize = 200;

/// Reject a routine `title` that carries no usable name with `400 Bad Request`.
///
/// `title` is the only required identifying field on a routine, yet it was never
/// content-checked. Two concrete failures follow from a blank or punctuation-only
/// title (issue #226):
///
/// 1. The moadim routine-origin disclosure breaks — `system_prompt_stmts` writes
///    `Routine name: <title>` into every workbench `CLAUDE.md`, so an empty title
///    yields a nameless disclosure the agent cannot satisfy.
/// 2. `slugify` maps any title with no ASCII-alphanumerics (`""`, `"   "`, `"!!!"`)
///    to the constant `"routine"`, so the routine silently takes a slug the user
///    never chose and collides with the next such routine.
///
/// Requiring at least one ASCII-alphanumeric character rejects all three cases at
/// once (it is exactly the condition under which `slugify` falls back). A max
/// length bounds downstream rendering. Shared by the create and update paths so
/// the REST and MCP surfaces reject identically, mirroring [`validate_cron`].
fn validate_title(title: &str) -> Result<(), AppError> {
    if !title.chars().any(|ch| ch.is_ascii_alphanumeric()) {
        return Err(AppError::BadRequest(
            "title must contain at least one alphanumeric character".to_string(),
        ));
    }
    if title.trim().chars().count() > MAX_TITLE_LEN {
        return Err(AppError::BadRequest(format!(
            "title must be at most {MAX_TITLE_LEN} characters"
        )));
    }
    Ok(())
}

/// Reject `repositories` entries whose URL (or set branch) is empty/whitespace-only, and return a
/// normalized copy with surrounding whitespace trimmed.
///
/// `repository` is a free-form string rendered verbatim into the agent's `prompt.md` preamble by
/// `compose_prompt` (see #241), so a blank or padded entry yields a broken `- ` clone bullet. An
/// empty list is valid — this only guards the contents of non-empty entries. Mirrors the
/// `validate_cron` / `validate_agent` boundary checks for the other routine fields (#224/#226).
fn validate_repositories(repos: &[Repository]) -> Result<Vec<Repository>, AppError> {
    let mut normalized = Vec::with_capacity(repos.len());
    for (index, repo) in repos.iter().enumerate() {
        let repository = repo.repository.trim();
        if repository.is_empty() {
            return Err(AppError::BadRequest(format!(
                "repositories[{index}].repository must not be empty or whitespace-only"
            )));
        }
        let branch = match &repo.branch {
            Some(branch) => {
                let trimmed = branch.trim();
                if trimmed.is_empty() {
                    return Err(AppError::BadRequest(format!(
                        "repositories[{index}].branch must not be empty or whitespace-only when set"
                    )));
                }
                Some(trimmed.to_string())
            }
            None => None,
        };
        normalized.push(Repository {
            repository: repository.to_string(),
            branch,
        });
    }
    Ok(normalized)
}

/// Validate `req`, assign a UUID, persist (routine.toml + prompt.md), and sync the crontab.
pub fn svc_create(
    store: &RoutineStore,
    req: CreateRoutineRequest,
) -> Result<RoutineResponse, AppError> {
    validate_cron(&req.schedule)?;
    reject_blank("title", &req.title)?;
    reject_blank("prompt", &req.prompt)?;
    reject_zero_secs("ttl_secs", req.ttl_secs)?;
    reject_zero_secs("max_runtime_secs", req.max_runtime_secs)?;
    validate_title(&req.title)?;
    validate_agent(&req.agent)?;
    let repositories = validate_repositories(&req.repositories)?;
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
        repositories,
        enabled: req.enabled,
        source: "managed".to_string(),
        created_at: now,
        updated_at: now,
        last_manual_trigger_at: None,
        last_scheduled_trigger_at: None,
        ttl_secs: req.ttl_secs,
        max_runtime_secs: req.max_runtime_secs,
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
    if let Some(ref title) = req.title {
        reject_blank("title", title)?;
        validate_title(title)?;
    }
    if let Some(ref prompt) = req.prompt {
        reject_blank("prompt", prompt)?;
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
    if let Some(repositories) = repositories {
        routine.repositories = repositories;
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
    routine.last_manual_trigger_at = Some(now_secs());
    let routine = routine.clone();
    drop(lock);
    write_routine(&routine).map_err(|_| AppError::Internal)?;
    match load_agent_command(&routine.agent) {
        Ok(agent) => {
            // A manual trigger records `last_manual_trigger_at` (above), not a scheduled fire, so
            // the launch script must not stamp the `scheduled.local.toml` sidecar.
            let cmd = build_routine_command(&routine, &agent, TriggerSource::Manual);
            // `-lc` (login shell) mirrors the crontab invocation (`/bin/sh -l <run.sh>`), so a
            // manual trigger sources the user's `~/.profile` and the agent gets the same
            // environment whether fired by cron or on demand.
            if let Err(err) = std::process::Command::new("sh")
                .arg("-lc")
                .arg(&cmd)
                .spawn()
            {
                log::warn!("trigger: failed to spawn routine command: {err}");
            }
        }
        Err(err) => log::warn!(
            "trigger: cannot load agent {:?} ({}) for routine {:?}",
            routine.agent,
            err,
            routine.id
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
