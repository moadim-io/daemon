//! Cross-platform in-process routine scheduling backed by `tokio-cron-scheduler`.
//!
//! Jobs are rebuilt after each routine mutation, while each actual fire goes through
//! [`crate::routines::svc_trigger_scheduled`] so the established global-lock, routine-lock,
//! snooze, power-saving, and same-minute deduplication rules remain authoritative.

#[cfg(not(test))]
use std::sync::OnceLock;

#[cfg(not(test))]
use tokio::sync::mpsc::{self, UnboundedSender};
#[cfg(not(test))]
use tokio_cron_scheduler::{Job, JobScheduler};
#[cfg(not(test))]
use uuid::Uuid;

#[cfg(not(test))]
use crate::routines::RoutineStore;

#[allow(
    clippy::missing_docs_in_private_items,
    reason = "the lifecycle seam is documented and tested from its sibling test module"
)]
#[path = "routine_scheduler_lifecycle.rs"]
mod lifecycle;

/// Request that the running scheduler rebuild its library-managed jobs.
#[cfg(not(test))]
pub(crate) fn request_resync() {
    if let Some(sender) = scheduler_resync_sender().get() {
        let _ = sender.send(());
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
