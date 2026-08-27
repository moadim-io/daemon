//! Cross-platform in-process routine scheduling backed by `tokio-cron-scheduler`.
//!
//! Jobs are rebuilt after each routine mutation, while each actual fire goes through
//! [`crate::routines::svc_trigger_scheduled`] so the established global-lock, routine-lock,
//! snooze, power-saving, and same-minute deduplication rules remain authoritative.

use std::sync::OnceLock;

use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tokio_cron_scheduler::{Job, JobScheduler};
use uuid::Uuid;

use crate::routines::{Routine, RoutineStore};
use crate::utils::lock::LockRecover;

/// Request that the running scheduler rebuild its library-managed jobs.
#[cfg(not(test))]
pub(crate) fn request_resync() {
    if let Some(sender) = scheduler_resync_sender().get() {
        let _ = sender.send(());
    }
}

/// Start a single scheduler actor for this daemon process.
pub(crate) fn spawn(store: RoutineStore) -> tokio::task::JoinHandle<()> {
    let (sender, receiver) = mpsc::unbounded_channel();
    if scheduler_resync_sender().set(sender).is_err() {
        log::warn!("routine scheduler is already running; ignoring duplicate start");
    }
    tokio::spawn(async move { run(store, receiver).await })
}

/// Hold the sender used by synchronous routine services to request a rebuild.
fn scheduler_resync_sender() -> &'static OnceLock<UnboundedSender<()>> {
    static SENDER: OnceLock<UnboundedSender<()>> = OnceLock::new();
    &SENDER
}

/// Run the library scheduler and rebuild its jobs when routine state changes.
async fn run(store: RoutineStore, mut receiver: UnboundedReceiver<()>) {
    let scheduler = match JobScheduler::new().await {
        Ok(scheduler) => scheduler,
        Err(err) => {
            log::error!("routine scheduler initialization failed: {err}");
            return;
        }
    };
    let mut job_ids = Vec::new();
    rebuild(&scheduler, &mut job_ids, &store).await;
    if let Err(err) = scheduler.start().await {
        log::error!("routine scheduler failed to start: {err}");
        return;
    }
    while receiver.recv().await.is_some() {
        // Coalesce all pending mutation notifications into one rebuild of the current store state.
        while receiver.try_recv().is_ok() {}
        rebuild(&scheduler, &mut job_ids, &store).await;
    }
}

/// Replace every registered job with jobs for the current enabled local routines.
async fn rebuild(scheduler: &JobScheduler, job_ids: &mut Vec<Uuid>, store: &RoutineStore) {
    for job_id in job_ids.drain(..) {
        if let Err(err) = scheduler.remove(&job_id).await {
            log::warn!("macOS routine scheduler could not remove job {job_id}: {err}");
        }
    }
    let machine = crate::machine::current_machine();
    let routines: Vec<Routine> = store
        .lock_recover()
        .values()
        .filter(|routine| {
            routine.source == "managed"
                && routine.enabled
                && crate::machine::targets(&routine.machines, &machine)
        })
        .cloned()
        .collect();
    for routine in &routines {
        for schedule in routine.effective_schedules() {
            let Ok(schedule) = to_scheduler_schedule(&schedule) else {
                log::warn!(
                    "macOS routine scheduler skipped invalid schedule {:?} for routine {:?}",
                    schedule,
                    routine.id
                );
                continue;
            };
            let store = store.clone();
            let routine_id = routine.id.clone();
            let job = Job::new_async_tz(&schedule, chrono::Local, move |_job_id, _scheduler| {
                let store = store.clone();
                let routine_id = routine_id.clone();
                Box::pin(async move {
                    let result = tokio::task::spawn_blocking(move || {
                        crate::routines::svc_trigger_scheduled(&store, &routine_id)
                    })
                    .await;
                    match result {
                        Ok(Ok(_routine)) => {}
                        Ok(Err(err)) => {
                            log::debug!("macOS routine scheduler skipped scheduled trigger: {err}");
                        }
                        Err(err) => {
                            log::warn!("macOS routine scheduler trigger task failed: {err}");
                        }
                    }
                })
            });
            match job {
                Ok(job) => match scheduler.add(job).await {
                    Ok(job_id) => job_ids.push(job_id),
                    Err(err) => log::warn!(
                        "macOS routine scheduler could not add schedule {:?} for routine {:?}: {err}",
                        schedule,
                        routine.id
                    ),
                },
                Err(err) => log::warn!(
                    "macOS routine scheduler could not build schedule {:?} for routine {:?}: {err}",
                    schedule,
                    routine.id
                ),
            }
        }
    }
}

/// Convert a Moadim five-field cron schedule into the library's required seconds-first form.
fn to_scheduler_schedule(schedule: &str) -> Result<String, &'static str> {
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

#[cfg(test)]
mod tests {
    use super::to_scheduler_schedule;
    use tokio_cron_scheduler::Job;

    #[test]
    fn converts_moadim_five_field_schedule_to_scheduler_seconds_format() {
        assert_eq!(
            to_scheduler_schedule("*/15 9-17 * * 1-5").unwrap(),
            "0 */15 9-17 * * 1-5"
        );
    }

    #[test]
    fn converted_five_field_schedule_is_accepted_by_the_scheduler_library() {
        let schedule = to_scheduler_schedule("30 9 * * 1-5").unwrap();
        assert!(Job::new(schedule, |_job_id, _scheduler| {}).is_ok());
    }

    #[test]
    fn converts_croner_keywords_to_scheduler_expressions() {
        assert_eq!(to_scheduler_schedule("@daily").unwrap(), "0 0 0 * * *");
        assert_eq!(to_scheduler_schedule("@weekly").unwrap(), "0 0 0 * * Sun");
    }

    #[test]
    fn rejects_a_non_five_field_schedule() {
        assert!(to_scheduler_schedule("0 0 9 * * 1-5").is_err());
    }
}
