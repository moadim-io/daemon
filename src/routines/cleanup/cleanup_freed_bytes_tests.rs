//! Tests for the disk-usage accounting behind `CleanupResponse.freed_bytes`: `dir_size`'s recursive
//! sum and its edge cases, and `reap_dir` reporting the freed total for a removed workbench. Split
//! out of `cleanup_tests.rs` to keep that file under the repo's line-count gate.

use super::*;

/// A `max_runtime_for` that never trips the watchdog (so reap tests exercise TTL only).
fn never_expires_runtime(_slug: &str) -> u64 {
    u64::MAX
}

/// A `kill` that does nothing (the watchdog is not expected to fire).
fn noop_kill(_session: &str) {}

/// A `finished_at` that reports each run's trigger timestamp as its finish time.
fn finish_at_trigger(_dir: &std::path::Path, trigger_ts: u64) -> u64 {
    trigger_ts
}

/// A `persist` that does nothing — these tests aren't exercising durable history.
fn noop_persist(
    _slug: &str,
    _name: &str,
    _path: &std::path::Path,
    _started_at: u64,
    _finished_at: u64,
) {
}

fn touch_dir(parent: &std::path::Path, name: &str) {
    std::fs::create_dir_all(parent.join(name)).unwrap();
}

#[test]
fn dir_size_returns_zero_for_a_missing_dir() {
    // Edge case: a path that does not exist (already removed, or never created) must not panic and
    // reads as 0 bytes rather than failing the whole sweep.
    let missing =
        std::env::temp_dir().join(format!("moadim-dir-size-missing-{}", uuid::Uuid::new_v4()));
    let _ = std::fs::remove_dir_all(&missing);
    assert_eq!(dir_size(&missing), 0);
}

#[test]
fn dir_size_returns_zero_for_an_empty_dir() {
    let base = std::env::temp_dir().join(format!("moadim-dir-size-empty-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&base).unwrap();
    assert_eq!(dir_size(&base), 0);
    std::fs::remove_dir_all(&base).unwrap();
}

#[test]
fn dir_size_sums_nested_files_recursively() {
    let base =
        std::env::temp_dir().join(format!("moadim-dir-size-nested-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(base.join("sub")).unwrap();
    std::fs::write(base.join("a.txt"), b"12345").unwrap(); // 5 bytes
    std::fs::write(base.join("sub").join("b.txt"), b"1234567890").unwrap(); // 10 bytes

    assert_eq!(dir_size(&base), 15);

    std::fs::remove_dir_all(&base).unwrap();
}

#[test]
fn reap_dir_reports_freed_bytes_for_removed_workbenches() {
    // The reaped workbench's file content size is summed into `freed_bytes`, measured before the
    // directory is deleted.
    let base = std::env::temp_dir().join(format!(
        "moadim-cleanup-freed-bytes-{}",
        uuid::Uuid::new_v4()
    ));
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(&base).unwrap();
    touch_dir(&base, "expired-100");
    std::fs::write(base.join("expired-100").join("agent.log"), vec![b'x'; 42]).unwrap();

    let now = 1000;
    let ttl_for = |_slug: &str| 500_u64;
    let dead = |_session: &str| false;

    let stats = reap_dir(
        &base,
        now,
        &ttl_for,
        &never_expires_runtime,
        &dead,
        &noop_kill,
        &finish_at_trigger,
        &noop_persist,
    );

    assert_eq!(stats.removed, 1);
    assert_eq!(stats.freed_bytes, 42);
    assert!(!base.join("expired-100").exists());

    std::fs::remove_dir_all(&base).unwrap();
}
