use tokio::sync::mpsc::UnboundedReceiver;
use uuid::Uuid;

use crate::routines::{Routine, RoutineStore};
use crate::utils::lock::LockRecover;

/// Operations the scheduler lifecycle needs from its job backend.
pub(super) trait Scheduler {
    /// Start dispatching already registered jobs.
    async fn start(&self) -> anyhow::Result<()>;
    /// Remove a job that belonged to the previous routine snapshot.
    async fn remove(&self, job_id: &Uuid) -> anyhow::Result<()>;
    /// Register a trigger for one converted routine schedule.
    async fn add(
        &self,
        schedule: &str,
        routine_id: String,
        store: RoutineStore,
    ) -> anyhow::Result<Uuid>;
}

/// Initialize a scheduler, then rebuild it after every routine mutation notification.
pub(super) async fn run<S: Scheduler>(
    scheduler: anyhow::Result<S>,
    store: RoutineStore,
    mut receiver: UnboundedReceiver<()>,
) {
    let scheduler = match scheduler {
        Ok(scheduler) => scheduler,
        Err(err) => {
            let error = format!("routine scheduler initialization failed: {err}");
            log::error!("{error}");
            crate::routine_scheduler::record_resync_result(Some(error));
            return;
        }
    };
    let mut job_ids = Vec::new();
    crate::routine_scheduler::record_resync_result(rebuild(&scheduler, &mut job_ids, &store).await);
    if let Err(err) = scheduler.start().await {
        let error = format!("routine scheduler failed to start: {err}");
        log::error!("{error}");
        crate::routine_scheduler::record_resync_result(Some(error));
        return;
    }
    while receiver.recv().await.is_some() {
        while receiver.try_recv().is_ok() {}
        crate::routine_scheduler::record_resync_result(
            rebuild(&scheduler, &mut job_ids, &store).await,
        );
    }
}

/// Replace the backend jobs with those for enabled, local, managed routines.
pub(super) async fn rebuild<S: Scheduler>(
    scheduler: &S,
    job_ids: &mut Vec<Uuid>,
    store: &RoutineStore,
) -> Option<String> {
    let mut last_error = None;
    for job_id in job_ids.drain(..) {
        if let Err(err) = scheduler.remove(&job_id).await {
            let error = format!("could not remove job {job_id}: {err}");
            log::warn!("routine scheduler {error}");
            last_error = Some(error);
        }
    }
    for routine in selected_routines(store) {
        for schedule in routine.effective_schedules() {
            let Ok(schedule) = to_scheduler_schedule(&schedule) else {
                let error = format!(
                    "skipped invalid schedule {:?} for routine {:?}",
                    schedule, routine.id
                );
                log::warn!("routine scheduler {error}");
                last_error = Some(error);
                continue;
            };
            match scheduler
                .add(&schedule, routine.id.clone(), store.clone())
                .await
            {
                Ok(job_id) => job_ids.push(job_id),
                Err(err) => {
                    let error = format!(
                        "could not add schedule {:?} for routine {:?}: {err}",
                        schedule, routine.id
                    );
                    log::warn!("routine scheduler {error}");
                    last_error = Some(error);
                }
            }
        }
    }
    last_error
}

/// Select only the routines that are eligible to execute on this daemon.
fn selected_routines(store: &RoutineStore) -> Vec<Routine> {
    let machine = crate::machine::current_machine();
    store
        .lock_recover()
        .values()
        .filter(|routine| {
            routine.source == "managed"
                && routine.enabled
                && crate::machine::targets(&routine.machines, &machine)
        })
        .cloned()
        .collect()
}

/// Convert a Moadim five-field cron schedule into the library's seconds-first form.
pub(super) fn to_scheduler_schedule(schedule: &str) -> Result<String, &'static str> {
    let trimmed = schedule.trim();
    let keyword = match trimmed {
        "@yearly" | "@annually" => Some("0 0 0 1 1 *"),
        "@monthly" => Some("0 0 0 1 * *"),
        "@weekly" => Some("0 0 0 * * Sun"),
        "@daily" | "@midnight" => Some("0 0 0 * * *"),
        "@hourly" => Some("0 0 * * * *"),
        _ => None,
    };
    if let Some(expanded) = keyword {
        return Ok(expanded.to_string());
    }
    if trimmed.starts_with('@') {
        return Err("unsupported cron keyword");
    }
    if trimmed.split_ascii_whitespace().count() != 5 {
        return Err("Moadim scheduler requires five cron fields");
    }
    Ok(format!("0 {trimmed}"))
}
