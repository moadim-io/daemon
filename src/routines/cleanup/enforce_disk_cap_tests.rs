#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use super::*;

fn touch_dir(parent: &std::path::Path, name: &str) {
    std::fs::create_dir_all(parent.join(name)).unwrap();
}

fn write_bytes(parent: &std::path::Path, name: &str, len: usize) {
    std::fs::write(parent.join(name).join("agent.log"), vec![b'x'; len]).unwrap();
}

/// A `finished_at` that reports each run's trigger timestamp as its finish time, isolating cap
/// eviction ordering from `agent.log` mtime.
fn finish_at_trigger(_dir: &std::path::Path, trigger_ts: u64) -> u64 {
    trigger_ts
}

#[test]
fn enforce_disk_cap_is_noop_when_unset() {
    let base =
        std::env::temp_dir().join(format!("moadim-enforce-cap-unset-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&base).unwrap();
    touch_dir(&base, "wb-100");

    let stats = enforce(&base, 0, &|_| false, &finish_at_trigger);
    assert_eq!(stats, ReapStats::default());
    assert!(base.join("wb-100").exists());

    std::fs::remove_dir_all(&base).unwrap();
}

#[test]
fn enforce_disk_cap_is_noop_for_a_missing_dir() {
    let missing = std::env::temp_dir().join(format!(
        "moadim-enforce-cap-missing-{}",
        uuid::Uuid::new_v4()
    ));
    let _ = std::fs::remove_dir_all(&missing);
    let stats = enforce(&missing, 100, &|_| false, &finish_at_trigger);
    assert_eq!(stats, ReapStats::default());
}

#[test]
fn enforce_disk_cap_evicts_oldest_finished_workbenches_over_cap() {
    let base =
        std::env::temp_dir().join(format!("moadim-enforce-cap-evict-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&base).unwrap();
    touch_dir(&base, "oldest-1");
    write_bytes(&base, "oldest-1", 40);
    touch_dir(&base, "newest-2");
    write_bytes(&base, "newest-2", 40);

    // Total 80 bytes over a 50-byte cap: the oldest-finished must be evicted, dropping to 40 <= 50.
    let stats = enforce(&base, 50, &|_| false, &finish_at_trigger);

    assert_eq!(stats.removed, 1);
    assert_eq!(stats.freed_bytes, 40);
    assert!(!base.join("oldest-1").exists());
    assert!(base.join("newest-2").exists());

    std::fs::remove_dir_all(&base).unwrap();
}

#[test]
fn enforce_disk_cap_never_evicts_a_live_session() {
    let base =
        std::env::temp_dir().join(format!("moadim-enforce-cap-live-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&base).unwrap();
    touch_dir(&base, "alive-1");
    write_bytes(&base, "alive-1", 100);

    // Even though the tree is far over cap, the only candidate is a live session, so nothing moves.
    let alive = |session: &str| session == "moadim-alive-1";
    let stats = enforce(&base, 10, &alive, &finish_at_trigger);

    assert_eq!(stats, ReapStats::default());
    assert!(base.join("alive-1").exists());

    std::fs::remove_dir_all(&base).unwrap();
}

#[cfg(unix)]
#[test]
fn enforce_disk_cap_counts_zero_when_remove_fails() {
    use std::os::unix::fs::PermissionsExt as _;

    // An over-cap, finished workbench whose removal fails (parent dir is read-only) is not
    // counted, exercising the `Err` arm of the eviction remove match.
    let base = std::env::temp_dir().join(format!(
        "moadim-enforce-cap-removefail-{}",
        uuid::Uuid::new_v4()
    ));
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(&base).unwrap();
    touch_dir(&base, "oldest-1");
    write_bytes(&base, "oldest-1", 100);

    // Strip write permission from the parent so removing the child directory fails.
    let mut perms = std::fs::metadata(&base).unwrap().permissions();
    perms.set_mode(0o555);
    std::fs::set_permissions(&base, perms).unwrap();

    let stats = enforce(&base, 10, &|_| false, &finish_at_trigger);
    // A read-only parent makes `remove_dir_all` fail for an unprivileged user, so the directory
    // survives and the Err arm runs (0 removed). Root bypasses the permission check; tolerate
    // that by only asserting consistency.
    if base.join("oldest-1").exists() {
        assert_eq!(stats.removed, 0);
    }

    // Restore permissions so cleanup can proceed.
    let mut perms = std::fs::metadata(&base).unwrap().permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(&base, perms).unwrap();
    std::fs::remove_dir_all(&base).unwrap();
}

#[test]
fn enforce_disk_cap_skips_a_file_and_a_non_workbench_dir() {
    let base =
        std::env::temp_dir().join(format!("moadim-enforce-cap-skip-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&base).unwrap();
    std::fs::write(base.join("stray-file"), b"x").unwrap();
    touch_dir(&base, "notawb");

    let stats = enforce(&base, 1, &|_| false, &finish_at_trigger);
    assert_eq!(stats, ReapStats::default());

    std::fs::remove_dir_all(&base).unwrap();
}
