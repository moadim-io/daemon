#![allow(
    clippy::wildcard_imports,
    clippy::too_many_arguments,
    clippy::needless_pass_by_value,
    dead_code,
    reason = "split-out module preserves existing code while keeping files under the linecheck limit"
)]
use super::*;

#[test]
fn enforce_repo_cache_cap_evicts_least_recently_fetched_mirrors_over_cap() {
    let base = std::env::temp_dir().join(format!(
        "moadim-repo-cache-enforce-evict-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&base).unwrap();
    touch_mirror(&base, "older-1");
    write_bytes(&base, "older-1", 40);
    touch_mirror(&base, "newer-2");
    write_bytes(&base, "newer-2", 40);

    // Total 80 bytes over a 50-byte cap: the least-recently-fetched must be evicted, dropping to
    // 40 <= 50.
    let stats = enforce(&base, 50, &last_fetch_from_name);

    assert_eq!(stats.removed, 1);
    assert_eq!(stats.freed_bytes, 40);
    assert!(!base.join("older-1").exists());
    assert!(base.join("newer-2").exists());

    std::fs::remove_dir_all(&base).unwrap();
}

#[test]
fn enforce_repo_cache_cap_is_noop_when_under_cap() {
    let base = std::env::temp_dir().join(format!(
        "moadim-repo-cache-enforce-under-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&base).unwrap();
    touch_mirror(&base, "mirror-1");
    write_bytes(&base, "mirror-1", 10);

    let stats = enforce(&base, 1000, &last_fetch_from_name);
    assert_eq!(stats, ReapStats::default());
    assert!(base.join("mirror-1").exists());

    std::fs::remove_dir_all(&base).unwrap();
}

#[test]
fn enforce_repo_cache_cap_skips_a_stray_file() {
    let base = std::env::temp_dir().join(format!(
        "moadim-repo-cache-enforce-skip-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&base).unwrap();
    std::fs::write(base.join("stray-file"), vec![b'x'; 100]).unwrap();

    // The stray file is not a directory, so it is skipped entirely — never counted toward
    // `total_bytes` nor considered for eviction, even far over cap.
    let stats = enforce(&base, 1, &last_fetch_from_name);
    assert_eq!(stats, ReapStats::default());
    assert!(base.join("stray-file").exists());

    std::fs::remove_dir_all(&base).unwrap();
}

#[cfg(unix)]
#[test]
fn enforce_repo_cache_cap_counts_zero_when_remove_fails() {
    use std::os::unix::fs::PermissionsExt as _;

    // An over-cap mirror whose removal fails (parent dir is read-only) is not counted, exercising
    // the `Err` arm of the eviction remove match.
    let base = std::env::temp_dir().join(format!(
        "moadim-repo-cache-enforce-removefail-{}",
        uuid::Uuid::new_v4()
    ));
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(&base).unwrap();
    touch_mirror(&base, "older-1");
    write_bytes(&base, "older-1", 100);

    // Strip write permission from the parent so removing the child directory fails.
    let mut perms = std::fs::metadata(&base).unwrap().permissions();
    perms.set_mode(0o555);
    std::fs::set_permissions(&base, perms).unwrap();

    let stats = enforce(&base, 10, &last_fetch_from_name);
    // A read-only parent makes `remove_dir_all` fail for an unprivileged user, so the directory
    // survives and the Err arm runs (0 removed). Root bypasses the permission check; tolerate
    // that by only asserting consistency.
    if base.join("older-1").exists() {
        assert_eq!(stats.removed, 0);
    }

    // Restore permissions so cleanup can proceed.
    let mut perms = std::fs::metadata(&base).unwrap().permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(&base, perms).unwrap();
    std::fs::remove_dir_all(&base).unwrap();
}

#[test]
fn mirror_last_fetch_time_falls_back_to_directory_mtime_when_fetch_head_missing() {
    let base = std::env::temp_dir().join(format!(
        "moadim-repo-cache-fetch-time-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&base).unwrap();

    // No `FETCH_HEAD` file exists, so the fallback to the directory's own mtime must not panic
    // and must yield a plausible (nonzero, roughly "now") unix timestamp.
    let now = crate::utils::time::now_secs();
    let fetched = mirror_last_fetch_time(&base);
    assert!(fetched > 0);
    assert!(fetched <= now + 5);

    std::fs::remove_dir_all(&base).unwrap();
}
