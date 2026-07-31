
#[test]
fn prune_orphaned_skips_a_stray_file() {
    let base = std::env::temp_dir().join(format!(
        "moadim-repo-cache-prune-file-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&base).unwrap();
    std::fs::write(base.join("stray-file"), b"x").unwrap();

    let stats = prune_orphaned(&base, &HashSet::new());
    assert_eq!(stats, ReapStats::default());
    assert!(base.join("stray-file").exists());

    std::fs::remove_dir_all(&base).unwrap();
}

#[cfg(unix)]
#[test]
fn prune_orphaned_counts_zero_when_remove_fails() {
    use std::os::unix::fs::PermissionsExt as _;

    // An orphaned mirror whose removal fails (parent dir is read-only) is not counted, exercising
    // the `Err` arm of the prune's remove match.
    let base = std::env::temp_dir().join(format!(
        "moadim-repo-cache-prune-removefail-{}",
        uuid::Uuid::new_v4()
    ));
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(&base).unwrap();
    touch_mirror(&base, "orphaned");
    write_bytes(&base, "orphaned", 10);

    // Strip write permission from the parent so removing the child directory fails.
    let mut perms = std::fs::metadata(&base).unwrap().permissions();
    perms.set_mode(0o555);
    std::fs::set_permissions(&base, perms).unwrap();

    let stats = prune_orphaned(&base, &HashSet::new());
    // A read-only parent makes `remove_dir_all` fail for an unprivileged user, so the directory
    // survives and the Err arm runs (0 removed). Root bypasses the permission check; tolerate
    // that by only asserting consistency.
    if base.join("orphaned").exists() {
        assert_eq!(stats.removed, 0);
    }

    // Restore permissions so cleanup can proceed.
    let mut perms = std::fs::metadata(&base).unwrap().permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(&base, perms).unwrap();
    std::fs::remove_dir_all(&base).unwrap();
}

#[test]
fn enforce_repo_cache_cap_is_noop_when_unset() {
    let base = std::env::temp_dir().join(format!(
        "moadim-repo-cache-enforce-unset-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&base).unwrap();
    touch_mirror(&base, "mirror-1");

    let stats = enforce(&base, 0, &last_fetch_from_name);
    assert_eq!(stats, ReapStats::default());
    assert!(base.join("mirror-1").exists());

    std::fs::remove_dir_all(&base).unwrap();
}

#[test]
fn enforce_repo_cache_cap_is_noop_for_a_missing_dir() {
    let missing = std::env::temp_dir().join(format!(
        "moadim-repo-cache-enforce-missing-{}",
        uuid::Uuid::new_v4()
    ));
    let _ = std::fs::remove_dir_all(&missing);
    let stats = enforce(&missing, 100, &last_fetch_from_name);
    assert_eq!(stats, ReapStats::default());
}

#[allow(
    clippy::missing_docs_in_private_items,
    reason = "split-out module keeps the file under the linecheck limit"
)]
#[path = "repo_cache_cap_tests_part2.rs"]
mod repo_cache_cap_tests_part2;
