//! Store-mutating service functions: list, get, create, update, delete, trigger, and logs.

use crate::utils::lock::LockRecover;
use uuid::Uuid;

use crate::error::AppError;
use crate::paths::workbenches_dir;
use crate::routine_storage::{remove_routine_dir, write_routine};
use crate::utils::cron::{normalize_schedule, validate_cron};
use crate::utils::time::now_secs;

use super::agents::{available_agents, load_agent_command, AgentLoadError};
use super::cleanup::{
    cleanup_expired_workbenches, max_runtime_ceiling_secs, parse_workbench_name, ttl_ceiling_secs,
};
use super::command::{build_routine_command, inline_prompt_overflow, slugify};
use super::flags::{self, Flag, FlagScope};
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

/// Reject a duration cap that exceeds the cron-derived `ceiling` for the routine's schedule.
///
/// `effective_ttl_secs` / `effective_max_runtime_secs` clamp an explicit value to
/// `min(MAX_*_SECS, cron interval)`, so a larger value is silently inert — accepted, persisted, and
/// shown in the UI, yet never enforced. Rejecting it up front (naming the ceiling) keeps the stored
/// config honest, mirroring the other `reject_*` / `validate_*` boundary checks (#468).
fn reject_over_ceiling(field: &str, value: Option<u64>, ceiling: u64) -> Result<(), AppError> {
    if let Some(secs) = value {
        if secs > ceiling {
            return Err(AppError::BadRequest(format!(
                "routine {field} {secs} exceeds the ceiling of {ceiling}s derived from this routine's schedule"
            )));
        }
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
        // An existing-but-unreadable config (e.g. permissions) would otherwise pass validation and
        // leave a green-dot routine that never fires; surface it now instead of silently dropping it.
        Err(AgentLoadError::Unreadable(err)) => Err(AppError::BadRequest(format!(
            "agent {agent:?} has an unreadable config: {err}"
        ))),
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

/// Reject a prompt that is empty or whitespace-only with `400 Bad Request`.
///
/// The prompt is the one field that defines what a routine actually does. A blank
/// prompt still produces a valid `prompt.compiled.md` (just the moadim preamble + repo list),
/// so the routine fires on every cron tick and launches an agent with no task —
/// silently burning scheduled runs and the user's agent/API budget (issue #224).
/// Shared by the create and update paths so the REST and MCP surfaces reject it
/// identically, mirroring [`validate_cron`].
fn validate_prompt(prompt: &str) -> Result<(), AppError> {
    if prompt.trim().is_empty() {
        return Err(AppError::BadRequest("prompt must not be empty".to_string()));
    }
    Ok(())
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
/// `repository` is a free-form string rendered verbatim into the agent's `prompt.compiled.md` preamble by
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

/// Reject blank (empty/whitespace-only) `tags` entries and return a normalized copy with each tag
/// trimmed.
///
/// Tags are free-form labels for grouping routines; an empty list is valid. This only guards the
/// contents of non-empty entries, mirroring [`validate_repositories`]: a blank label carries no
/// meaning and would render as an empty chip, so it is refused at edit time rather than stored.
fn validate_tags(tags: &[String]) -> Result<Vec<String>, AppError> {
    let mut normalized = Vec::with_capacity(tags.len());
    for (index, tag) in tags.iter().enumerate() {
        let trimmed = tag.trim();
        if trimmed.is_empty() {
            return Err(AppError::BadRequest(format!(
                "tags[{index}] must not be empty or whitespace-only"
            )));
        }
        normalized.push(trimmed.to_string());
    }
    Ok(normalized)
}

/// Reject blank (empty/whitespace-only) `machines` entries and return a normalized copy with each
/// entry trimmed and duplicates collapsed (first occurrence kept).
///
/// `machine::targets` matches this list by exact string equality against the resolved machine name
/// (see #600). Left unvalidated, a whitespace-padded or typo'd entry can never match anything, and a
/// non-empty list of *only* empty-string entries slips past the dormant-routine warning — which fires
/// solely on `machines.is_empty()` — leaving a routine that runs nowhere with no warning at all.
/// Trimming and rejecting blanks mirrors `validate_repositories`/`validate_tags`; the extra dedup
/// step additionally stops `"host"` and `" host "` from persisting as if they targeted two machines.
fn validate_machines(machines: &[String]) -> Result<Vec<String>, AppError> {
    let mut normalized: Vec<String> = Vec::with_capacity(machines.len());
    for (index, machine) in machines.iter().enumerate() {
        let trimmed = machine.trim();
        if trimmed.is_empty() {
            return Err(AppError::BadRequest(format!(
                "machines[{index}] must not be empty or whitespace-only"
            )));
        }
        if !normalized.iter().any(|existing| existing == trimmed) {
            normalized.push(trimmed.to_string());
        }
    }
    Ok(normalized)
}

/// Normalize an optional model ID: trims it and collapses blank/whitespace-only input to `None`, so
/// a cleared text field on the create/edit form is stored as "no override" rather than an empty
/// string.
fn normalize_model(model: Option<String>) -> Option<String> {
    model.and_then(|model| {
        let trimmed = model.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

/// Maximum number of lines a routine `goal` may span. The goal is meant to be a glanceable "why"
/// rendered as a `## Goal` preamble in `prompt.md`, not a second prompt, so it is capped short.
const MAX_GOAL_LINES: usize = 5;

/// Normalize and bound an optional routine `goal`, returning the value to store.
///
/// The goal is a very short statement of *why* a routine exists, rendered into the agent's
/// `prompt.md` as a `## Goal` preamble. It is optional: a `None` or blank (empty/whitespace-only)
/// value clears it (`Ok(None)`). A present goal is trimmed and must span at most
/// [`MAX_GOAL_LINES`] lines, so it stays a glanceable summary rather than a second prompt. Shared
/// by the create and update paths so the REST and MCP surfaces bound it identically.
fn validate_goal(goal: Option<&str>) -> Result<Option<String>, AppError> {
    let Some(trimmed) = goal.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    if trimmed.lines().count() > MAX_GOAL_LINES {
        return Err(AppError::BadRequest(format!(
            "goal must be at most {MAX_GOAL_LINES} lines"
        )));
    }
    Ok(Some(trimmed.to_string()))
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
        ttl_secs: req.ttl_secs,
        max_runtime_secs: req.max_runtime_secs,
        tags,
    };
    write_routine(&routine).map_err(|_| AppError::Internal)?;
    store
        .lock_recover()
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
pub fn svc_delete(store: &RoutineStore, id: &str) -> Result<RoutineResponse, AppError> {
    let routine = store.lock_recover().remove(id).ok_or(AppError::NotFound)?;
    remove_routine_dir(&slugify(&routine.title)).map_err(|_| AppError::Internal)?;
    if let Err(err) = crate::sync::routines::sync_routines_to_crontab(store) {
        log::warn!("crontab sync after routine delete failed: {err}");
    }
    Ok(RoutineResponse::from_routine(routine))
}

/// Record a manual trigger for `id` and spawn the same command the crontab would run.
pub fn svc_trigger(store: &RoutineStore, id: &str) -> Result<Routine, AppError> {
    if crate::global_lock::is_globally_locked() {
        return Err(AppError::Locked("routines are globally locked".into()));
    }
    let mut lock = store.lock_recover();
    let routine = lock.get_mut(id).ok_or(AppError::NotFound)?;
    routine.last_manual_trigger_at = Some(now_secs());
    let routine = routine.clone();
    drop(lock);
    write_routine(&routine).map_err(|_| AppError::Internal)?;
    spawn_routine_command(&routine);
    Ok(routine)
}

/// Run a routine on its schedule: spawn the command the crontab line invokes, without recording a
/// *manual* trigger.
///
/// This is the daemon-side endpoint that the generated crontab line drives
/// (`moadim schedule trigger <id>`). Unlike [`svc_trigger`] it leaves `last_manual_trigger_at`
/// untouched — the spawned command records `last_scheduled_trigger_at` in the routine's
/// `scheduled.local.toml` sidecar itself, which the daemon reads back on the next load. Keeping the
/// two paths distinct preserves the manual-vs-scheduled distinction the timestamps exist to capture.
///
/// A routine snoozed via [`svc_snooze`] (`snoozed_until` in the future, or `skip_runs` above zero)
/// is skipped here instead of spawned: `snoozed_until` clears itself once elapsed (that fire then
/// runs), `skip_runs` decrements once per skipped fire and clears at zero. [`svc_trigger`] (manual)
/// ignores both fields entirely, by design.
pub fn svc_trigger_scheduled(store: &RoutineStore, id: &str) -> Result<Routine, AppError> {
    if crate::global_lock::is_globally_locked() {
        return Err(AppError::Locked("routines are globally locked".into()));
    }
    let mut lock = store.lock_recover();
    let routine = lock.get_mut(id).ok_or(AppError::NotFound)?;

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
fn sh_bin() -> String {
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

/// Spawn the launch command for `routine` under a login shell, logging (rather than failing) when
/// the agent config cannot be loaded, the composed prompt won't fit in an inlined `{prompt}`
/// argument, or the process cannot be spawned.
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

/// Reap finished, expired run workbenches immediately, returning how many were removed.
///
/// Runs the same sweep as the hourly background task ([`cleanup_expired_workbenches`]) but on
/// demand, so callers need not wait for the next tick. Still-running sessions are never touched.
pub fn svc_cleanup(store: &RoutineStore) -> CleanupResponse {
    CleanupResponse {
        removed: cleanup_expired_workbenches(store),
    }
}

/// Rename every existing workbench directory from `old_slug` to `new_slug`, preserving each run's
/// trigger timestamp (`{old_slug}-{ts}` -> `{new_slug}-{ts}`).
///
/// Called from [`svc_update`] when a routine's title (and thus slug) changes. Workbenches are keyed
/// by slug, not the routine's stable UUID, so without this migration a rename would strand every
/// prior run under the old slug: [`svc_logs`] (which looks up by *current* slug) would find nothing,
/// and an in-flight run would fall through to the cleanup watchdog's orphan defaults instead of the
/// routine's own `ttl_secs`/`max_runtime_secs` (#267). A failed rename is logged and skipped rather
/// than failing the update itself — this is best-effort history preservation, not a correctness
/// requirement of the rename.
fn migrate_workbenches(old_slug: &str, new_slug: &str) {
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

/// Return the contents of the newest workbench `agent.log` for routine `id`.
pub fn svc_logs(store: &RoutineStore, id: &str) -> Result<String, AppError> {
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
        return Ok(String::new());
    };
    let log_path = workbenches_dir().join(dir).join("agent.log");
    if !log_path.exists() {
        return Ok(String::new());
    }
    std::fs::read_to_string(&log_path).map_err(|_| AppError::Internal)
}

/// Reject a blank (empty/whitespace-only) flag `type` or `description`.
fn validate_flag_field(field: &str, value: &str) -> Result<(), AppError> {
    if value.trim().is_empty() {
        return Err(AppError::BadRequest(format!(
            "flag {field} must not be empty"
        )));
    }
    Ok(())
}

/// Parse a `scope` string into a [`FlagScope`], returning `400 BadRequest` on unknown values.
/// Mirrors `parse_lock_scope` in `handlers.rs`.
fn parse_flag_scope(scope: &str) -> Result<FlagScope, AppError> {
    match scope {
        "general" => Ok(FlagScope::General),
        "local" => Ok(FlagScope::Local),
        other => Err(AppError::BadRequest(format!(
            "unknown flag scope {other:?}; use \"general\" or \"local\""
        ))),
    }
}

/// Look up a routine by `id` and derive its slug, `NotFound` if it does not exist.
fn routine_and_slug(store: &RoutineStore, id: &str) -> Result<(Routine, String), AppError> {
    let routine = store
        .lock_recover()
        .get(id)
        .cloned()
        .ok_or(AppError::NotFound)?;
    let slug = slugify(&routine.title);
    Ok((routine, slug))
}

/// Raise a new flag against routine `id`. `flag_type` and `description` must be non-blank;
/// `scope` is `"general"` (committed) or `"local"` (gitignored). Refreshes the routine's
/// `prompts/prompt.compiled.md` afterward so the next run's "Open flags" section (see
/// `compose_prompt`) includes it.
pub fn svc_create_flag(
    store: &RoutineStore,
    id: &str,
    flag_type: &str,
    description: &str,
    scope: &str,
) -> Result<Flag, AppError> {
    validate_flag_field("type", flag_type)?;
    validate_flag_field("description", description)?;
    let scope = parse_flag_scope(scope)?;
    let (routine, slug) = routine_and_slug(store, id)?;
    let flag =
        flags::create_flag(&slug, flag_type, description, scope).map_err(|_| AppError::Internal)?;
    write_routine(&routine).map_err(|_| AppError::Internal)?;
    Ok(flag)
}

/// List every open flag raised against routine `id`, oldest first.
pub fn svc_list_flags(store: &RoutineStore, id: &str) -> Result<Vec<Flag>, AppError> {
    let (_, slug) = routine_and_slug(store, id)?;
    Ok(flags::list_flags(&slug))
}

/// Resolve (delete) the flag named `filename` under routine `id`.
///
/// `NotFound` when the routine does not exist, `filename` is unsafe, or names no existing flag.
/// Refreshes `prompts/prompt.compiled.md` afterward so a resolved flag stops appearing in the next
/// run's prompt.
pub fn svc_resolve_flag(store: &RoutineStore, id: &str, filename: &str) -> Result<(), AppError> {
    let (routine, slug) = routine_and_slug(store, id)?;
    let resolved = flags::resolve_flag(&slug, filename).map_err(|_| AppError::Internal)?;
    if !resolved {
        return Err(AppError::NotFound);
    }
    write_routine(&routine).map_err(|_| AppError::Internal)?;
    Ok(())
}

#[cfg(test)]
#[path = "service_tests.rs"]
mod service_tests;

#[cfg(test)]
#[path = "service_flag_tests.rs"]
mod service_flag_tests;
