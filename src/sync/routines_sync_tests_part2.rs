#![allow(
    clippy::wildcard_imports,
    clippy::too_many_arguments,
    clippy::needless_pass_by_value,
    dead_code,
    reason = "split-out module preserves existing code while keeping files under the linecheck limit"
)]
use super::*;

#[test]
fn build_block_orders_tied_created_at_by_id_deterministically() {
    // Two enabled managed routines sharing a created_at must emit in a stable, id-ordered
    // sequence regardless of HashMap iteration order, so the generated crontab block does not
    // churn across syncs. Insert in id-descending order to prove the sort — not insertion or
    // hash order — fixes the line order.
    let agent_name = "test-sync-agent-tied-order";
    std::fs::create_dir_all(crate::paths::agents_dir()).unwrap();
    let cfg = crate::paths::agent_toml_path(agent_name);
    std::fs::write(&cfg, "command = \"claude\"\nargs = []\n").unwrap();

    let title_a = "Tied Order Alpha Routine";
    let title_b = "Tied Order Beta Routine";
    let slug_a = slugify(title_a);
    let slug_b = slugify(title_b);

    let store = new_store();
    // id "b-tied" > "a-tied"; both created_at == 0 (the make_routine default).
    store
        .lock()
        .unwrap()
        .insert("b-tied".into(), make_routine("b-tied", title_b, agent_name));
    store
        .lock()
        .unwrap()
        .insert("a-tied".into(), make_routine("a-tied", title_a, agent_name));

    let block = build_block(&store);
    let pos_a = block.find("# moadim-routine:a-tied").unwrap();
    let pos_b = block.find("# moadim-routine:b-tied").unwrap();
    assert!(pos_a < pos_b, "lower id must sort first: {block}");
    // Stable across repeated builds.
    assert_eq!(block, build_block(&store));

    std::fs::remove_file(&cfg).unwrap();
    let _ = std::fs::remove_dir_all(crate::paths::routine_dir(&slug_a));
    let _ = std::fs::remove_dir_all(crate::paths::routine_dir(&slug_b));
}

#[test]
fn build_block_includes_routine_with_agent_config() {
    let agent_name = "test-sync-agent-build-block";
    let title = "Inc Sync Routine";
    let slug = slugify(title);
    std::fs::create_dir_all(crate::paths::agents_dir()).unwrap();
    let cfg = crate::paths::agent_toml_path(agent_name);
    std::fs::write(&cfg, "command = \"claude\"\nargs = []\n").unwrap();

    let store = new_store();
    store
        .lock()
        .unwrap()
        .insert("inc".into(), make_routine("inc", title, agent_name));
    let block = build_block(&store);
    assert!(block.contains("# moadim-routine:inc"));
    assert!(block.contains("schedule trigger 'inc'"));

    std::fs::remove_file(&cfg).unwrap();
    let _ = std::fs::remove_dir_all(crate::paths::routine_dir(&slug));
}

#[test]
fn build_block_excludes_routine_targeting_another_machine() {
    let agent_name = "test-sync-agent-other-machine";
    std::fs::create_dir_all(crate::paths::agents_dir()).unwrap();
    let cfg = crate::paths::agent_toml_path(agent_name);
    std::fs::write(&cfg, "command = \"claude\"\nargs = []\n").unwrap();

    let store = new_store();
    let mut routine = make_routine("other", "Other Machine Routine", agent_name);
    // Assigned to a machine that is not this host: it must not be scheduled here.
    routine.machines = vec!["definitely-not-this-host-zzz".to_string()];
    store.lock().unwrap().insert("other".into(), routine);
    let block = build_block(&store);
    assert!(!block.contains("moadim-routine:"));

    std::fs::remove_file(&cfg).unwrap();
}

#[test]
fn build_block_skips_routine_with_no_machine_assignment() {
    let store = new_store();
    // Empty `machines` means the routine runs nowhere — it is dormant and excluded (and logged as
    // such via `warn_dormant_routines`).
    let mut routine = make_routine("dormant", "Dormant Routine", "claude");
    routine.machines = vec![];
    store.lock().unwrap().insert("dormant".into(), routine);
    let block = build_block(&store);
    assert!(!block.contains("moadim-routine:"));
}

#[test]
fn build_block_empty_when_globally_locked() {
    let agent_name = "test-sync-agent-global-lock";
    let title = "Global Lock Sync Routine";
    let slug = crate::routines::slugify(title);
    std::fs::create_dir_all(crate::paths::agents_dir()).unwrap();
    let cfg = crate::paths::agent_toml_path(agent_name);
    std::fs::write(&cfg, "command = \"claude\"\nargs = []\n").unwrap();

    let store = new_store();
    store.lock().unwrap().insert(
        "lock-test".into(),
        make_routine("lock-test", title, agent_name),
    );

    // Create the shared lock sentinel and verify it suppresses all crontab lines.
    let lock_path = crate::paths::global_lock_path();
    if let Some(parent) = lock_path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(&lock_path, b"").unwrap();

    let block = build_block(&store);
    assert!(
        !block.contains("moadim-routine:"),
        "locked block must have no routine lines"
    );
    assert!(block.contains(BLOCK_BEGIN));
    assert!(block.contains(BLOCK_END));

    std::fs::remove_file(&lock_path).unwrap();
    std::fs::remove_file(&cfg).unwrap();
    let _ = std::fs::remove_dir_all(crate::paths::routine_dir(&slug));
}

#[test]
fn crontab_sync_lock_is_mutually_exclusive() {
    // Same static `Mutex` instance every call: while held here, a second attempt to acquire it
    // must fail instead of silently locking a *different* mutex.
    let _guard = crontab_sync_lock().lock_recover();
    assert!(
        crontab_sync_lock().try_lock().is_err(),
        "crontab_sync_lock() must return the same process-wide mutex on every call"
    );
}

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
