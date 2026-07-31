#![allow(
    clippy::wildcard_imports,
    clippy::too_many_arguments,
    clippy::needless_pass_by_value,
    dead_code,
    reason = "split-out module preserves existing code while keeping files under the linecheck limit"
)]
use super::*;

/// Return the routines matching `query`, filtered and sorted as requested.
///
/// The default query (no repository filter, sort by creation time ascending)
/// reproduces the previous behaviour, except each routine's `prompt` is omitted
/// unless `include_prompts` is `true`. The `repository` filter keeps routines
/// referencing a matching repository URL; `sort`/`order` control ordering.
pub fn svc_list(
    store: &RoutineStore,
    dir: &std::path::Path,
    query: &RoutineListQuery,
) -> Vec<RoutineResponse> {
    // Refresh from disk first so a routine pulled/edited on disk under a running daemon (including a
    // changed `machines` list) is reflected without a restart. Disk is the source of truth.
    crate::routine_storage::reload_store_from_dir(store, dir);
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
pub fn svc_get(
    store: &RoutineStore,
    dir: &std::path::Path,
    id: &str,
) -> Result<RoutineResponse, AppError> {
    // Refresh from disk first so a freshly-pulled or edited routine is visible without a restart.
    crate::routine_storage::reload_store_from_dir(store, dir);
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
    let input_schedules = if req.schedules.is_empty() {
        vec![req.schedule.clone()]
    } else {
        req.schedules.clone()
    };
    let schedules = validate_and_normalize_schedules(&input_schedules)?;
    reject_blank("title", &req.title)?;
    validate_prompt(&req.prompt)?;
    reject_zero_secs("ttl_secs", req.ttl_secs)?;
    reject_zero_secs("max_runtime_secs", req.max_runtime_secs)?;
    reject_over_ceiling(
        "ttl_secs",
        req.ttl_secs,
        min_schedule_ceiling(&schedules, ttl_ceiling_secs),
    )?;
    reject_over_ceiling(
        "max_runtime_secs",
        req.max_runtime_secs,
        min_schedule_ceiling(&schedules, max_runtime_ceiling_secs),
    )?;
    validate_title(&req.title)?;
    validate_agent(&req.agent)?;
    let repositories = validate_repositories(&req.repositories)?;
    let tags = validate_tags(&req.tags)?;
    let goal = validate_goal(req.goal.as_deref())?;
    let machines = validate_machines(&req.machines)?;
    validate_env(&req.env)?;
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
        schedule: schedules[0].clone(),
        schedules,
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
        // A brand-new routine has no run history yet; the circuit-breaker state (like
        // `power_saving`) starts clean regardless of `failure_threshold`.
        consecutive_failures: 0,
        auto_disabled_reason: None,
        ttl_secs: req.ttl_secs,
        max_runtime_secs: req.max_runtime_secs,
        failure_threshold: req.failure_threshold,
        tags,
        env: req.env,
    };
    write_routine(&routine).map_err(|err| map_write_routine_err(&err))?;
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
