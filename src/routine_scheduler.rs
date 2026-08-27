//! macOS-native, in-process dispatch of routine cron schedules.
//!
//! macOS does not use the OS crontab for routines. The daemon checks every 15 seconds and passes
//! due routine IDs through [`crate::routines::svc_trigger_scheduled`], preserving its existing
//! locking, snooze, power-saving, and same-minute deduplication behavior.

use chrono::{DateTime, Duration, Local};
use croner::Cron;

use crate::routines::{Routine, RoutineStore};
use crate::utils::lock::LockRecover;

/// How often the macOS daemon checks routine schedules.
pub(crate) const ROUTINE_SCHEDULER_INTERVAL: std::time::Duration =
    std::time::Duration::from_secs(15);

/// How far back a scheduler tick searches for a cron fire.
const ROUTINE_SCHEDULER_LOOKBACK: Duration = Duration::seconds(90);

/// Start the macOS routine scheduler as a daemon-owned background task.
#[cfg(target_os = "macos")]
pub(crate) fn spawn(store: RoutineStore) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(ROUTINE_SCHEDULER_INTERVAL);
        loop {
            tick.tick().await;
            let store = store.clone();
            let joined = tokio::task::spawn_blocking(move || trigger_due_routines(&store)).await;
            if let Err(err) = joined {
                log::warn!("macOS routine scheduler task failed: {err}");
            }
        }
    })
}

/// Identify then trigger every enabled routine whose local cron schedule fired recently.
#[cfg(target_os = "macos")]
fn trigger_due_routines(store: &RoutineStore) {
    let now = Local::now();
    let machine = crate::machine::current_machine();
    let routines: Vec<Routine> = store.lock_recover().values().cloned().collect();
    for id in due_routine_ids(&routines, now - ROUTINE_SCHEDULER_LOOKBACK, now, &machine) {
        if let Err(err) = crate::routines::svc_trigger_scheduled(store, &id) {
            log::debug!("macOS routine scheduler skipped {id:?}: {err}");
        }
    }
}

/// Return IDs of enabled local routines with a fire in `(window_start, now]`.
///
/// Croner evaluates against [`Local`], matching the OS-crontab timezone semantics this scheduler
/// replaces. Each matching routine contributes only one ID even when its schedules overlap; the
/// trigger service supplies the durable cross-tick same-minute deduplication.
fn due_routine_ids(
    routines: &[Routine],
    window_start: DateTime<Local>,
    now: DateTime<Local>,
    machine: &str,
) -> Vec<String> {
    routines
        .iter()
        .filter(|routine| {
            routine.source == "managed"
                && routine.enabled
                && crate::machine::targets(&routine.machines, machine)
        })
        .filter(|routine| {
            routine.effective_schedules().iter().any(|schedule| {
                schedule
                    .parse::<Cron>()
                    .ok()
                    .is_some_and(|cron| cron.iter_after(window_start).any(|fire| fire <= now))
            })
        })
        .map(|routine| routine.id.clone())
        .collect()
}

#[cfg(test)]
#[path = "routine_scheduler_tests.rs"]
mod routine_scheduler_tests;
