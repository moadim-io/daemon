use std::sync::{Arc, Mutex};

use super::lifecycle::{rebuild, run, to_scheduler_schedule, Scheduler};
use super::scheduler_resync_status;
use crate::routines::RoutineStore;
use crate::utils::lock::LockRecover;
use tokio::sync::mpsc;
use uuid::Uuid;

#[derive(Clone, Default)]
struct FakeScheduler {
    state: Arc<Mutex<FakeState>>,
}

#[derive(Default)]
struct FakeState {
    add_calls: Vec<(String, String)>,
    remove_calls: Vec<Uuid>,
    start_error: bool,
    remove_error: bool,
    add_error: bool,
}

impl Scheduler for FakeScheduler {
    async fn start(&self) -> anyhow::Result<()> {
        if self.state.lock().expect("fake state poisoned").start_error {
            anyhow::bail!("start failed");
        }
        Ok(())
    }

    async fn remove(&self, job_id: &Uuid) -> anyhow::Result<()> {
        let mut state = self.state.lock().expect("fake state poisoned");
        state.remove_calls.push(*job_id);
        if state.remove_error {
            anyhow::bail!("remove failed");
        }
        Ok(())
    }

    async fn add(
        &self,
        schedule: &str,
        routine_id: String,
        _store: RoutineStore,
    ) -> anyhow::Result<Uuid> {
        let mut state = self.state.lock().expect("fake state poisoned");
        state.add_calls.push((schedule.to_string(), routine_id));
        if state.add_error {
            anyhow::bail!("add failed");
        }
        Ok(Uuid::new_v4())
    }
}

fn store_with_scheduler_cases() -> RoutineStore {
    let store = RoutineStore::default();
    let mut valid = crate::test_fixtures::routine_fixture("scheduled", "Scheduled").build();
    valid.schedules = vec!["@daily".to_string(), "not a real cron".to_string()];
    let mut disabled = crate::test_fixtures::routine_fixture("disabled", "Disabled")
        .enabled(false)
        .build();
    disabled.schedule = "@hourly".to_string();
    let mut unmanaged = crate::test_fixtures::routine_fixture("unmanaged", "Unmanaged").build();
    unmanaged.source = "external".to_string();
    let mut remote = crate::test_fixtures::routine_fixture("remote", "Remote").build();
    remote.machines = vec!["another-machine".to_string()];
    let mut routines = store.lock_recover();
    routines.insert(valid.id.clone(), valid);
    routines.insert(disabled.id.clone(), disabled);
    routines.insert(unmanaged.id.clone(), unmanaged);
    routines.insert(remote.id.clone(), remote);
    drop(routines);
    store
}

#[test]
fn converts_moadim_schedules_and_rejects_unsupported_forms() {
    assert_eq!(
        to_scheduler_schedule("*/15 9-17 * * 1-5").expect("five fields convert"),
        "0 */15 9-17 * * 1-5"
    );
    assert_eq!(
        to_scheduler_schedule("@daily").expect("daily converts"),
        "0 0 0 * * *"
    );
    assert_eq!(
        to_scheduler_schedule("@weekly").expect("weekly converts"),
        "0 0 0 * * Sun"
    );
    assert_eq!(
        to_scheduler_schedule("@yearly").expect("yearly converts"),
        "0 0 0 1 1 *"
    );
    assert_eq!(
        to_scheduler_schedule("@monthly").expect("monthly converts"),
        "0 0 0 1 * *"
    );
    assert_eq!(
        to_scheduler_schedule("@hourly").expect("hourly converts"),
        "0 0 * * * *"
    );
    assert!(to_scheduler_schedule("@reboot").is_err());
    assert!(to_scheduler_schedule("0 0 9 * * 1-5").is_err());
}

#[tokio::test]
async fn rebuild_registers_only_eligible_routines_and_tolerates_backend_failures() {
    let store = store_with_scheduler_cases();
    let scheduler = FakeScheduler::default();
    let mut job_ids = Vec::new();
    let _ = rebuild(&scheduler, &mut job_ids, &store).await;
    assert_eq!(job_ids.len(), 1);
    assert_eq!(
        scheduler
            .state
            .lock()
            .expect("fake state poisoned")
            .add_calls
            .len(),
        1
    );

    let _ = rebuild(&scheduler, &mut job_ids, &store).await;
    assert_eq!(job_ids.len(), 1);
    assert_eq!(
        scheduler
            .state
            .lock()
            .expect("fake state poisoned")
            .remove_calls
            .len(),
        1
    );

    scheduler
        .state
        .lock()
        .expect("fake state poisoned")
        .remove_error = true;
    scheduler
        .state
        .lock()
        .expect("fake state poisoned")
        .add_error = true;
    let _ = rebuild(&scheduler, &mut job_ids, &store).await;
    let state = scheduler.state.lock().expect("fake state poisoned");
    assert_eq!(state.remove_calls.len(), 2);
    assert_eq!(state.add_calls.len(), 3);
}

#[tokio::test]
async fn lifecycle_handles_initialization_and_start_failures() {
    let store = RoutineStore::default();
    let (_sender, receiver) = mpsc::unbounded_channel();
    run::<FakeScheduler>(
        Err(anyhow::anyhow!("initialization failed")),
        store.clone(),
        receiver,
    )
    .await;

    let scheduler = FakeScheduler::default();
    scheduler
        .state
        .lock()
        .expect("fake state poisoned")
        .start_error = true;
    let (_sender, receiver) = mpsc::unbounded_channel();
    run(Ok(scheduler), store, receiver).await;
}

#[tokio::test]
async fn lifecycle_coalesces_resync_notifications_before_rebuilding() {
    let store = store_with_scheduler_cases();
    let scheduler = FakeScheduler::default();
    let (sender, receiver) = mpsc::unbounded_channel();
    sender.send(()).expect("actor receives first notification");
    sender.send(()).expect("actor receives second notification");
    drop(sender);
    run(Ok(scheduler.clone()), store, receiver).await;
    assert_eq!(
        scheduler
            .state
            .lock()
            .expect("fake state poisoned")
            .add_calls
            .len(),
        2
    );
    let status = scheduler_resync_status();
    assert!(!status.ok);
    assert!(status.last_completed_at.is_some());
    assert_eq!(
        status.last_error.as_deref(),
        Some("skipped invalid schedule \"not a real cron\" for routine \"scheduled\"")
    );
    assert!(status.last_error_at.is_some());
    super::record_resync_result(None);
}
