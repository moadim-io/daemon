
#[test]
fn sync_routines_to_crontab_serializes_concurrent_calls() {
    // Regression test for #365: two `sync_routines_to_crontab` calls whose crontab I/O overlaps
    // must not interleave. Proven by timing rather than by racing on store content: the shim
    // sleeps a fixed delay on every `crontab -` write, so two *unserialized* read-modify-write
    // round trips would overlap and finish in roughly one delay; serialized by the lock, they run
    // back to back and take roughly two.
    const DELAY_MS: u32 = 150;

    let agent_name = "test-sync-agent-concurrent-lock";
    std::fs::create_dir_all(crate::paths::agents_dir()).unwrap();
    let cfg = crate::paths::agent_toml_path(agent_name);
    std::fs::write(&cfg, "command = \"claude\"\nargs = []\n").unwrap();

    let shim = CronShim::new_with_write_delay(
        "# BEGIN MOADIM-ROUTINES\n# END MOADIM-ROUTINES\n",
        DELAY_MS,
    );

    let store_a = new_store();
    store_a.lock().unwrap().insert(
        "lock-a".into(),
        make_routine("lock-a", "Lock A Sync Routine", agent_name),
    );
    let store_b = new_store();
    store_b.lock().unwrap().insert(
        "lock-b".into(),
        make_routine("lock-b", "Lock B Sync Routine", agent_name),
    );

    let start = std::time::Instant::now();
    std::thread::scope(|scope| {
        scope.spawn(|| sync_routines_to_crontab(&store_a).unwrap());
        scope.spawn(|| sync_routines_to_crontab(&store_b).unwrap());
    });
    let elapsed = start.elapsed();

    assert!(
        elapsed.as_millis() >= u128::from(DELAY_MS) * 2,
        "two concurrent syncs completed in {elapsed:?}, faster than {}ms — the crontab lock did \
         not serialize their read-modify-write round trips",
        DELAY_MS * 2,
    );

    drop(shim);
    std::fs::remove_file(&cfg).unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sync_routines_to_crontab_runs_via_block_in_place_on_multi_thread_runtime() {
    // Regression test for #360: on a multi-thread runtime `sync_routines_to_crontab` must opt
    // into `tokio::task::block_in_place` (not just run the same blocking body inline) so a slow
    // `crontab` subprocess can't tie up a worker thread other tasks are scheduled on. Called
    // directly from `async fn` (not via `spawn_blocking`), mirroring how a real async handler
    // (`routines::handlers::lock`/`unlock`, etc.) calls this synchronous function. `block_in_place`
    // panics outright on a `current_thread` runtime, so a bare pass here — under
    // `#[tokio::test(flavor = "multi_thread")]` — already shows the flavor check took the
    // block-in-place branch instead of skipping it.
    let agent_name = "test-sync-agent-multi-thread";
    std::fs::create_dir_all(crate::paths::agents_dir()).unwrap();
    let cfg = crate::paths::agent_toml_path(agent_name);
    std::fs::write(&cfg, "command = \"claude\"\nargs = []\n").unwrap();

    let shim = CronShim::new("# BEGIN MOADIM-ROUTINES\n# END MOADIM-ROUTINES\n");
    let store = new_store();
    store.lock().unwrap().insert(
        "mt".into(),
        make_routine("mt", "Multi Thread Sync Routine", agent_name),
    );

    sync_routines_to_crontab(&store).unwrap();
    assert!(shim.store_contents().contains("# moadim-routine:mt"));

    drop(shim);
    std::fs::remove_file(&cfg).unwrap();
}
