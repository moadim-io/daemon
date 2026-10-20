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
