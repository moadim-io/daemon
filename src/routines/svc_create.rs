
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
    let timezone = validate_timezone(req.timezone.as_deref())?;
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
        disabled_reason: (!req.enabled).then_some(req.disabled_reason).flatten(),
        source: "managed".to_string(),
        created_at: now,
        updated_at: now,
        last_manual_trigger_at: None,
        last_scheduled_trigger_at: None,
        snoozed_until: None,
        skip_runs: None,
        // Power saving is system-driven, never settable via explicit state on create/update — see
        // `svc_set_power_saving`. The exemption is user-owned config and may be set on create.
        power_saving: false,
        power_saving_exempt: req.power_saving_exempt,
        // A brand-new routine has no run history yet; the circuit-breaker state (like
        // `power_saving`) starts clean regardless of `failure_threshold`.
        consecutive_failures: 0,
        auto_disabled_reason: None,
        ttl_secs: req.ttl_secs,
        max_runtime_secs: req.max_runtime_secs,
        failure_threshold: req.failure_threshold,
        notifications: req.notifications,
        tags,
        env: req.env,
        timezone,
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
