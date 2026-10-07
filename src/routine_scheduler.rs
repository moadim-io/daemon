//! Cross-platform in-process routine scheduling backed by `tokio-cron-scheduler`.
//!
//! Jobs are rebuilt after each routine mutation, while each actual fire goes through
//! [`crate::routines::svc_trigger_scheduled`] so the established global-lock, routine-lock,
//! snooze, power-saving, and same-minute deduplication rules remain authoritative.

use std::sync::{Mutex, OnceLock};

#[cfg(not(test))]
use tokio::sync::mpsc::{self, UnboundedSender};
#[cfg(not(test))]
use tokio_cron_scheduler::{Job, JobScheduler};
#[cfg(not(test))]
use uuid::Uuid;

#[cfg(not(test))]
use crate::routines::RoutineStore;
use crate::utils::lock::LockRecover;
use crate::utils::time::now_secs;

#[allow(
    clippy::missing_docs_in_private_items,
    reason = "the lifecycle seam is documented and tested from its sibling test module"
)]
#[path = "routine_scheduler_lifecycle.rs"]
mod lifecycle;

/// Process-local state for the most recent in-process scheduler rebuild.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct SchedulerResyncStatus {
    /// Whether the most recently completed rebuild had no invalid schedules or backend failures.
    pub ok: bool,
    /// Unix timestamp of the most recently completed rebuild.
    pub last_completed_at: Option<u64>,
    /// Description of the most recent rebuild problem, if any.
    pub last_error: Option<String>,
    /// Unix timestamp of the most recent rebuild problem, if any.
    pub last_error_at: Option<u64>,
}

/// Return the latest in-process scheduler rebuild status for health reporting.
pub(crate) fn scheduler_resync_status() -> SchedulerResyncStatus {
    scheduler_resync_state().lock_recover().clone()
}

/// Record the outcome of a completed in-process scheduler rebuild.
pub(super) fn record_resync_result(error: Option<String>) {
    let now = now_secs();
    let failed = error.is_some();
    *scheduler_resync_state().lock_recover() = SchedulerResyncStatus {
        ok: !failed,
        last_completed_at: Some(now),
        last_error: error,
        last_error_at: failed.then_some(now),
    };
}

/// Hold in-process scheduler status independently of the OS-crontab health state.
fn scheduler_resync_state() -> &'static Mutex<SchedulerResyncStatus> {
    static STATE: OnceLock<Mutex<SchedulerResyncStatus>> = OnceLock::new();
    STATE.get_or_init(|| {
        Mutex::new(SchedulerResyncStatus {
            ok: true,
            ..SchedulerResyncStatus::default()
        })
    })
}

/// Request that the running scheduler rebuild its library-managed jobs.
#[cfg(not(test))]
pub(crate) fn request_resync() {
    let Some(sender) = scheduler_resync_sender().get() else {
        let error = "routine scheduler is not running".to_string();
        log::warn!("routine scheduler resync request failed: {error}");
        record_resync_result(Some(error));
        return;
    };
    if sender.send(()).is_err() {
        let error = "routine scheduler actor has stopped".to_string();
        log::warn!("routine scheduler resync request failed: {error}");
        record_resync_result(Some(error));
    }
}

/// Start a single scheduler actor for this daemon process.
#[cfg(not(test))]
pub(crate) fn spawn(store: RoutineStore) -> tokio::task::JoinHandle<()> {
    let (sender, receiver) = mpsc::unbounded_channel();
    if scheduler_resync_sender().set(sender).is_err() {
        log::warn!("routine scheduler is already running; ignoring duplicate start");
    }
    tokio::spawn(
        async move { lifecycle::run(ProductionScheduler::new().await, store, receiver).await },
    )
}

/// Hold the sender used by synchronous routine services to request a rebuild.
#[cfg(not(test))]
fn scheduler_resync_sender() -> &'static OnceLock<UnboundedSender<()>> {
    static SENDER: OnceLock<UnboundedSender<()>> = OnceLock::new();
    &SENDER
}

/// Adapter from the scheduler library to the lifecycle orchestration seam.
#[cfg(not(test))]
struct ProductionScheduler {
    /// Library instance that owns the registered routine jobs.
    scheduler: JobScheduler,
}

#[cfg(not(test))]
impl ProductionScheduler {
    /// Create the library scheduler before the actor accepts mutation notifications.
    async fn new() -> anyhow::Result<Self> {
        Ok(Self {
            scheduler: JobScheduler::new().await?,
        })
    }
}

#[cfg(not(test))]
impl lifecycle::Scheduler for ProductionScheduler {
    async fn start(&self) -> anyhow::Result<()> {
        self.scheduler.start().await?;
        Ok(())
    }

    async fn remove(&self, job_id: &Uuid) -> anyhow::Result<()> {
        self.scheduler.remove(job_id).await?;
        Ok(())
    }

    async fn add(
        &self,
        schedule: &str,
        routine_id: String,
        store: RoutineStore,
    ) -> anyhow::Result<Uuid> {
        let job = Job::new_async_tz(schedule, chrono::Local, move |_job_id, _scheduler| {
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
                        log::debug!("routine scheduler skipped scheduled trigger: {err}");
                    }
                    Err(err) => log::warn!("routine scheduler trigger task failed: {err}"),
                }
            })
        })?;
        Ok(self.scheduler.add(job).await?)
    }
}

#[cfg(test)]
#[path = "routine_scheduler_tests.rs"]
mod routine_scheduler_tests;
