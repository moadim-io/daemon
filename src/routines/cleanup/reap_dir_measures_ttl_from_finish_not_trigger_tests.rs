
#[test]
fn reap_dir_measures_ttl_from_finish_not_trigger() {
    // #174: retention is measured from when the run *finished*, not when it was triggered. A run
    // whose duration exceeds its TTL must still be kept for the full window after it completes,
    // while a long-finished run is reaped. Both dirs share trigger ts 100 (trigger-based age 900),
    // so a trigger-based reaper would delete both; finish-based keeps the just-finished one.
    let base = std::env::temp_dir().join("moadim-cleanup-finish-ttl-test");
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(&base).unwrap();

    touch_dir(&base, "longrun-100"); // triggered at 100, finished recently (at 900)
    touch_dir(&base, "donelong-100"); // triggered at 100, finished long ago (at 100)

    let now = 1000;
    let ttl_for = |_slug: &str| 500_u64; // retention window: 500s from finish
    let dead = |_session: &str| false;
    // Finish time is per-workbench: the long-running one finished at 900 (age 100 <= 500 -> kept);
    // the other finished at 100 (age 900 > 500 -> reaped). Run duration never eats the window.
    let finished_at = |dir: &std::path::Path, _ts: u64| {
        if dir.file_name().unwrap() == "longrun-100" {
            900
        } else {
            100
        }
    };

    let stats = reap_dir(
        &base,
        now,
        &ttl_for,
        &never_expires_runtime,
        &dead,
        &noop_kill,
        &finished_at,
        &noop_persist,
    );

    assert_eq!(stats.removed, 1, "only the long-finished run is reaped");
    assert!(
        base.join("longrun-100").exists(),
        "a run that finished within its TTL is retained even though its trigger age exceeds the TTL"
    );
    assert!(!base.join("donelong-100").exists());

    std::fs::remove_dir_all(&base).unwrap();
}

#[test]
fn agent_log_finish_time_falls_back_to_trigger_without_log() {
    // No agent.log present -> the trigger timestamp is used as the finish time.
    let base =
        std::env::temp_dir().join(format!("moadim-cleanup-finishfn-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&base).unwrap();
    assert_eq!(agent_log_finish_time(&base, 4242), 4242);
    std::fs::remove_dir_all(&base).unwrap();
}

#[test]
fn agent_log_finish_time_uses_log_mtime_clamped_to_trigger() {
    // With an agent.log present, its mtime (a recent, large unix time) is used and is never less
    // than the trigger timestamp.
    let base = std::env::temp_dir().join(format!(
        "moadim-cleanup-finishfn-mtime-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&base).unwrap();
    std::fs::write(base.join("agent.log"), b"done\n").unwrap();
    // Trigger far in the past: the just-written log's mtime dominates, so finish > trigger.
    let finish = agent_log_finish_time(&base, 1);
    assert!(
        finish > 1,
        "fresh agent.log mtime should yield a finish time later than an ancient trigger"
    );
    // Trigger far in the future (clock skew): clamped up to the trigger, never below it.
    assert_eq!(agent_log_finish_time(&base, u64::MAX), u64::MAX);
    std::fs::remove_dir_all(&base).unwrap();
}

// Run-history persistence coverage (`cleanup_expired_workbenches` + `runs.log`) lives in
// `cleanup_run_history_tests.rs`.

// `dir_size`/`freed_bytes` coverage lives in `cleanup_freed_bytes_tests.rs`.

#[allow(
    clippy::missing_docs_in_private_items,
    reason = "split-out module keeps the file under the linecheck limit"
)]
#[path = "reap_dir_kills_hung_session_over_max_runtime_then_reaps_tests.rs"]
mod reap_dir_kills_hung_session_over_max_runtime_then_reaps_tests;
