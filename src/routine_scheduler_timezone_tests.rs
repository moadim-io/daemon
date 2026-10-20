#[tokio::test]
async fn rebuild_passes_each_routine_timezone_and_skips_unknown_ones() {
    let store = RoutineStore::default();
    let mut pinned = crate::test_fixtures::routine_fixture("pinned", "Pinned").build();
    pinned.timezone = Some("Asia/Tokyo".to_string());
    let local = crate::test_fixtures::routine_fixture("local", "Local").build();
    let mut bogus = crate::test_fixtures::routine_fixture("bogus", "Bogus").build();
    bogus.timezone = Some("Mars/Olympus".to_string());
    for routine in [pinned, local, bogus] {
        store.lock_recover().insert(routine.id.clone(), routine);
    }
    let scheduler = FakeScheduler::default();
    let error = rebuild(&scheduler, &mut Vec::new(), &store).await;
    assert!(error.is_some_and(|error| error.contains("Mars/Olympus")));
    let mut calls: Vec<(Option<chrono_tz::Tz>, String)> = scheduler
        .state
        .lock()
        .expect("fake state poisoned")
        .add_calls
        .iter()
        .map(|(_, timezone, id)| (*timezone, id.clone()))
        .collect();
    calls.sort_by(|left, right| left.1.cmp(&right.1));
    assert_eq!(
        calls,
        vec![
            (None, "local".to_string()),
            (Some(chrono_tz::Asia::Tokyo), "pinned".to_string()),
        ]
    );
}
