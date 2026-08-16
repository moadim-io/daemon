
/// Reconcile an existing default `cur` against its built-in `spec`, preserving the user's choices.
///
/// Returns `Some(updated)` when a daemon-owned field (schedule, agent, prompt, goal, or the empty
/// repositories list) drifted from the spec and the routine must be rewritten, or `None` when `cur`
/// already matches and no write is needed. The user-owned [`Routine::enabled`] toggle is always
/// carried over from `cur` — so a default the user turned off stays off — as are its `id`,
/// `created_at`, `last_manual_trigger_at`, `last_scheduled_trigger_at`, `snoozed_until`,
/// `skip_runs`, `tags`, and `env`.
///
/// Special case: if `cur.machines` is empty the routine is dormant and can never run. This is the
/// legacy state for defaults seeded before machine-awareness was added. To repair it, an empty
/// machines list is treated as a drift trigger and replaced with the current machine, matching what
/// [`materialize`] does for freshly created defaults. (#723)
fn reconcile(spec: &DefaultRoutine, cur: &Routine, now: u64) -> Option<Routine> {
    let schedule = normalize_schedule(spec.schedule);
    let schedules = vec![schedule.clone()];
    let up_to_date = cur.schedule == schedule
        && cur.effective_schedules() == schedules
        && cur.agent == spec.agent
        && cur.prompt == spec.prompt
        && cur.goal.as_deref() == Some(spec.goal)
        && cur.repositories.is_empty()
        // An empty machines list means the routine can never run; treat it as drift so the
        // current machine is seeded and the routine becomes active again (#723).
        && !cur.machines.is_empty();
    if up_to_date {
        return None;
    }
    Some(Routine {
        id: cur.id.clone(),
        schedule,
        schedules,
        title: spec.title.to_string(),
        agent: spec.agent.to_string(),
        // Model is user-owned, like `tags`: never overridden by the spec.
        model: cur.model.clone(),
        prompt: spec.prompt.to_string(),
        goal: Some(spec.goal.to_string()),
        repositories: Vec::new(),
        // Machine targeting is user-owned, like `enabled`: carry the existing choice across a
        // spec-driven reconcile so a default reassigned (or unassigned) by the user stays that
        // way. Exception: an empty list means the routine is dormant (legacy pre-machine-awareness
        // state); seed the current machine so it starts running out of the box (#723).
        machines: if cur.machines.is_empty() {
            vec![crate::machine::current_machine()]
        } else {
            cur.machines.clone()
        },
        enabled: cur.enabled,
        disabled_reason: None,
        source: "managed".to_string(),
        created_at: cur.created_at,
        updated_at: now,
        last_manual_trigger_at: cur.last_manual_trigger_at,
        last_scheduled_trigger_at: cur.last_scheduled_trigger_at,
        // Snooze state is daemon-owned but not spec-derived: carry it over so a reconcile (e.g. a
        // prompt tweak upstream) doesn't silently wake a routine the agent chose to snooze.
        snoozed_until: cur.snoozed_until,
        skip_runs: cur.skip_runs,
        // Power saving is daemon/policy-owned, not spec-derived: carry it over like snooze state.
        power_saving: cur.power_saving,
        power_saving_exempt: cur.power_saving_exempt,
        // Circuit-breaker runtime state is daemon-owned but not spec-derived either: carry it over
        // like power saving, so a reconcile doesn't silently clear an in-progress failure streak or
        // an auto-disable reason.
        consecutive_failures: cur.consecutive_failures,
        auto_disabled_reason: cur.auto_disabled_reason.clone(),
        ttl_secs: cur.ttl_secs,
        max_runtime_secs: cur.max_runtime_secs,
        // The circuit-breaker threshold is user-owned, like `tags`: never overridden by the spec.
        failure_threshold: cur.failure_threshold,
        notifications: cur.notifications.clone(),
        // Tags are user-owned, like `enabled`: never overridden by the spec.
        tags: cur.tags.clone(),
        // Env vars are user-owned, like `tags`: never overridden by the spec.
        env: cur.env.clone(),
        // Timezone is user-owned, like `tags`/`env`: never overridden by the spec.
        timezone: cur.timezone.clone(),
    })
}

/// Ensure every built-in default routine exists and matches its spec, then schedule it.
///
/// For each [`DEFAULT_ROUTINES`] entry: if a routine with the same slug is already in `store`, it is
/// refreshed via [`reconcile`] (daemon-owned content updated, the user's `enabled` toggle preserved)
/// and only rewritten when it drifted; otherwise a fresh enabled routine is created. Persists each
/// affected routine (`routine.toml` + `prompts/` sidecars + `.gitignore`) and inserts it into `store` so the
/// subsequent crontab sync schedules it. Best-effort: a write failure is logged and skipped rather
/// than aborting startup. Call once at startup after [`crate::routine_storage::load_store`] and
/// before the crontab sync.
pub fn ensure_default_routines(store: &RoutineStore) {
    let removed = read_removed_defaults();
    for spec in DEFAULT_ROUTINES {
        let slug = slugify(spec.title);
        let existing = store
            .lock_recover()
            .values()
            .find(|routine| slugify(&routine.title) == slug)
            .cloned();
        let routine = if let Some(cur) = existing {
            match reconcile(spec, &cur, now_secs()) {
                Some(updated) => updated,
                None => continue,
            }
        } else {
            if removed.contains(&slug) {
                continue;
            }
            materialize(spec, now_secs())
        };
        if let Err(err) = write_routine(&routine) {
            log::warn!(
                "ensure_default_routines: failed to write {:?}: {err}; skipping",
                spec.title
            );
            continue;
        }
        store.lock_recover().insert(routine.id.clone(), routine);
    }
}

/// True when `slug` matches one of the built-in [`DEFAULT_ROUTINES`].
#[must_use]
pub fn is_default_slug(slug: &str) -> bool {
    DEFAULT_ROUTINES
        .iter()
        .any(|spec| slugify(spec.title) == slug)
}

/// On-disk shape of the [`removed_default_routines_path`] tombstone file.
#[derive(Default, Serialize, Deserialize)]
struct RemovedDefaults {
    /// Slugs of built-in defaults the user has explicitly deleted.
    #[serde(default)]
    slugs: BTreeSet<String>,
}

/// Read the set of tombstoned default slugs, or an empty set if the file is absent or unreadable.
fn read_removed_defaults() -> BTreeSet<String> {
    let Ok(raw) = std::fs::read_to_string(removed_default_routines_path()) else {
        return BTreeSet::new();
    };
    toml::from_str::<RemovedDefaults>(&raw)
        .map(|removed| removed.slugs)
        .unwrap_or_default()
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod defaults_tests;

#[cfg(test)]
#[path = "lock_tests.rs"]
mod defaults_lock_tests;
