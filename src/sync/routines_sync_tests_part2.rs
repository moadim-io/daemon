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
include!("routines_sync_tests_part4.rs");
