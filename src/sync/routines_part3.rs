
/// Build the full routines block from the enabled managed routines in `store`.
///
/// Only routines assigned to *this* machine ([`crate::machine::current_machine`]) are scheduled: a
/// shared config repo can drive different routines on different machines. A routine with an empty
/// `machines` list runs nowhere — these are logged once as dormant so the operator notices an
/// unassigned routine instead of it silently never firing. Routines whose agent config is missing
/// are skipped with a warning.
fn build_block(store: &RoutineStore) -> String {
    if crate::global_lock::is_globally_locked() {
        log::info!("routine sync: global lock active — clearing all routine crontab lines");
        return format!("{BLOCK_BEGIN}\n{BLOCK_HEADER}\n{BLOCK_END}");
    }
    let me = crate::machine::current_machine();
    let mut routines: Vec<Routine> = {
        let lock = store.lock_recover();
        lock.values()
            .filter(|routine| routine.source == "managed" && routine.enabled)
            .cloned()
            .collect()
    };
    warn_dormant_routines(&routines);
    routines.retain(|routine| crate::machine::targets(&routine.machines, &me));
    // The routines come off a `HashMap`, whose iteration order is unspecified, so routines that
    // share a `created_at` (e.g. several seeded or batch-created in the same second) would otherwise
    // emit in an arbitrary, run-to-run order. That churns the generated crontab block across syncs
    // and defeats the `new_crontab == current` idempotency guard below, forcing a needless
    // `crontab -` rewrite. Break ties on the stable routine id so the block is fully deterministic.
    routines.sort_by(|left, right| {
        left.created_at
            .cmp(&right.created_at)
            .then_with(|| left.id.cmp(&right.id))
    });

    let lines: Vec<String> = routines
        .iter()
        .filter_map(|routine| match load_agent_command(&routine.agent) {
            // Validate the agent config at sync time so a broken routine is skipped here rather than
            // failing at fire time; the crontab line itself no longer embeds the agent command.
            Ok(_) => Some({
                let pure_schedules = pure_schedules_for_crontab(routine);
                let compailed_schedules = compailed_schedules_for_crontab(routine, &pure_schedules);
                write_compailed_cron_sidecar(routine, &compailed_schedules);
                compailed_schedules
                    .iter()
                    .map(|schedule| format_routine_line_for_schedule(routine, schedule))
                    .collect::<Vec<_>>()
            }),
            Err(err) => {
                log::warn!(
                    "routine sync: cannot load agent {:?} ({}) for routine {:?}; skipping",
                    routine.agent,
                    err,
                    routine.id
                );
                None
            }
        })
        .flatten()
        .collect();

    if lines.is_empty() {
        format!("{BLOCK_BEGIN}\n{BLOCK_HEADER}\n{BLOCK_END}")
    } else {
        format!(
            "{BLOCK_BEGIN}\n{BLOCK_HEADER}\n{}\n{BLOCK_END}",
            lines.join("\n")
        )
    }
}

/// Log a single warning naming enabled routines with no machine assignment (empty `machines`).
///
/// With "unset targeting = runs nowhere", such routines never schedule on any machine. Surfacing
/// them once at sync time makes that visible (e.g. after an upgrade from a version without
/// targeting) instead of leaving the operator to wonder why a routine never fires.
fn warn_dormant_routines(routines: &[Routine]) {
    let dormant: Vec<&str> = routines
        .iter()
        .filter(|routine| routine.machines.is_empty())
        .map(|routine| routine.title.as_str())
        .collect();
    if !dormant.is_empty() {
        log::warn!(
            "{} enabled routine(s) have no machine assignment and will not be scheduled on any \
             machine: {}; assign with `moadim routines update <id> --machines '[\"<name>\"]'`",
            dormant.len(),
            dormant.join(", ")
        );
    }
}

#[cfg(test)]
#[path = "routines_sync_tests.rs"]
mod routines_sync_tests;

#[cfg(test)]
#[path = "routines_sync_status_tests.rs"]
mod routines_sync_status_tests;

#[cfg(test)]
#[path = "routines_sync_multi_cron_tests.rs"]
mod routines_sync_multi_cron_tests;
