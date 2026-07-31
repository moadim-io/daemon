
#[cfg(target_os = "macos")]
#[test]
fn launchctl_bin_never_resolves_to_real_launchctl_in_test_builds() {
    // Structural guard for issue #213: in a test build, with no `MOADIM_LAUNCHCTL_BIN`
    // shim configured, `launchctl_bin()` must never fall back to the real `launchctl`,
    // so a test that forgets to isolate launchctl cannot mutate the developer's live
    // launchd session. The resolved path must also not exist, so the eventual spawn
    // fails harmlessly. Mirrors `crontab_bin_never_resolves_to_real_crontab_in_test_builds`.
    let previous = std::env::var_os("MOADIM_LAUNCHCTL_BIN");
    // SAFETY: tests run single-threaded (RUST_TEST_THREADS=1); restored below.
    unsafe {
        std::env::remove_var("MOADIM_LAUNCHCTL_BIN");
    }
    let bin = launchctl_bin();
    // SAFETY: single-threaded harness; restore the saved value if any.
    unsafe {
        match previous {
            Some(value) => std::env::set_var("MOADIM_LAUNCHCTL_BIN", value),
            None => std::env::remove_var("MOADIM_LAUNCHCTL_BIN"),
        }
    }
    assert_ne!(
        bin, "launchctl",
        "test build must not fall back to the real launchctl"
    );
    assert!(
        !std::path::Path::new(&bin).exists(),
        "the test-build launchctl guard path must not exist so the spawn fails: {bin}"
    );
}

#[cfg(target_os = "macos")]
#[test]
fn plist_path_errors_when_home_is_unknown() {
    // Covers the `ok_or_else` error arm of `plist_path_from_home` (home directory undeterminable).
    assert!(plist_path_from_home(None).is_err());
    // And the happy path resolves under the given home.
    let path = plist_path_from_home(Some(std::path::PathBuf::from("/home/u"))).unwrap();
    assert!(path.ends_with("Library/LaunchAgents/io.moadim.daemon.plist"));
}

#[cfg(target_os = "macos")]
#[test]
fn write_plist_skips_dir_creation_when_paths_have_no_parent() {
    // Exercises the `None` arm of the defensive `if let Some(dir) = .parent()` guards: a parent-less
    // path ("") skips create_dir_all for both the plist and the log. The trailing write then fails,
    // which is expected — only the no-parent branches need exercising.
    let no_parent = std::path::Path::new("");
    assert!(write_plist(no_parent, no_parent, no_parent, no_parent, no_parent).is_err());
}

#[cfg(target_os = "macos")]
#[test]
fn write_plist_errors_when_plist_dir_creation_blocked() {
    // Covers the `?` error branch at the first create_dir_all (plist parent dir).
    // A regular file sitting where LaunchAgents/ should be prevents create_dir_all.
    let base = std::env::temp_dir().join(format!("moadim-wp-plist-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&base).unwrap();
    // Create a FILE at the LaunchAgents path, blocking directory creation.
    std::fs::write(base.join("LaunchAgents"), "block").unwrap();
    let plist = base.join("LaunchAgents/io.moadim.daemon.plist");
    let log = base.join("daemon.log");
    assert!(write_plist(
        &plist,
        std::path::Path::new("/usr/local/bin/moadim"),
        &log,
        &base,
        &base
    )
    .is_err());
    let _ = std::fs::remove_dir_all(&base);
}

#[cfg(target_os = "macos")]
#[test]
fn write_plist_errors_when_log_dir_creation_blocked() {
    // Covers the `?` error branch at the second create_dir_all (log parent dir).
    // The plist dir succeeds, but a file blocks the log dir creation.
    let base = std::env::temp_dir().join(format!("moadim-wp-log-{}", uuid::Uuid::new_v4()));
    let launch_agents = base.join("Library/LaunchAgents");
    std::fs::create_dir_all(&launch_agents).unwrap();
    // Block the log parent directory with a file.
    let log_parent = base.join("logparent");
    std::fs::write(&log_parent, "block").unwrap();
    let plist = launch_agents.join("io.moadim.daemon.plist");
    // Give a log path whose parent is the blocked non-directory.
    let log = log_parent.join("daemon.log");
    assert!(write_plist(
        &plist,
        std::path::Path::new("/usr/local/bin/moadim"),
        &log,
        &base,
        &base
    )
    .is_err());
    let _ = std::fs::remove_dir_all(&base);
}

#[allow(
    clippy::missing_docs_in_private_items,
    reason = "split-out module keeps the file under the linecheck limit"
)]
#[path = "mod_tests_part2.rs"]
mod mod_tests_part2;
