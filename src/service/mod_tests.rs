#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

#[cfg(target_os = "macos")]
use super::*;

#[cfg(target_os = "macos")]
#[test]
fn plist_carries_label_program_args_and_supervision_keys() {
    let plist = render_plist(
        std::path::Path::new("/opt/moadim/bin/moadim"),
        std::path::Path::new("/Users/u/.config/moadim/daemon.log"),
        std::path::Path::new("/Users/u"),
        std::path::Path::new("/Users/u/.hermes"),
    );
    assert!(plist.contains("<string>io.moadim.daemon</string>"));
    assert!(plist.contains("<string>/opt/moadim/bin/moadim</string>"));
    assert!(plist.contains("<string>--interactive</string>"));
    assert!(plist.contains("<key>RunAtLoad</key>"));
    // KeepAlive is failure-only (a `{ SuccessfulExit = false }` dict, not unconditional `true`), so
    // a clean `moadim stop` is not resurrected by launchd while a crash still restarts (#444).
    assert!(plist.contains("<key>KeepAlive</key>"));
    assert!(plist.contains("<key>SuccessfulExit</key>"));
    assert!(
        !plist.contains("<key>KeepAlive</key>\n  <true/>"),
        "KeepAlive must not be unconditional true"
    );
    assert!(plist.contains("/Users/u/.config/moadim/daemon.log"));
    assert!(plist.contains("<key>WorkingDirectory</key>"));
    assert!(plist.contains("<string>/Users/u/.hermes</string>"));
    assert!(plist.contains("<key>EnvironmentVariables</key>"));
    assert!(plist.contains("/opt/homebrew/bin:/usr/local/bin:/Users/u/.cargo/bin"));
}

#[cfg(target_os = "macos")]
#[test]
fn plist_escapes_xml_metacharacters_in_paths() {
    let plist = render_plist(
        std::path::Path::new("/tmp/a&b<c>"),
        std::path::Path::new("/tmp/log"),
        std::path::Path::new("/tmp/home"),
        std::path::Path::new("/tmp/work&root"),
    );
    assert!(plist.contains("/tmp/a&amp;b&lt;c&gt;"));
    assert!(plist.contains("/tmp/work&amp;root"));
    assert!(!plist.contains("a&b<c>"));
}

#[cfg(target_os = "macos")]
#[test]
fn xml_escape_covers_all_five_metacharacters() {
    assert_eq!(xml_escape("&<>\"'"), "&amp;&lt;&gt;&quot;&apos;");
}

#[cfg(target_os = "macos")]
#[test]
fn plist_path_is_under_launch_agents() {
    let path = plist_path().unwrap();
    assert!(path.ends_with("Library/LaunchAgents/io.moadim.daemon.plist"));
}

#[cfg(target_os = "macos")]
#[test]
fn plist_path_honors_home_override() {
    // With `MOADIM_HOME_OVERRIDE` set (as the install/uninstall tests do), `plist_path()` must
    // resolve under the temp home, never the developer's real `~/Library/LaunchAgents`.
    let base = std::env::temp_dir().join(format!("moadim-plist-home-{}", uuid::Uuid::new_v4()));
    let prev_override = std::env::var_os("MOADIM_HOME_OVERRIDE");
    // SAFETY: tests run single-threaded (RUST_TEST_THREADS=1); the var is restored below.
    unsafe {
        std::env::set_var("MOADIM_HOME_OVERRIDE", &base);
    }

    let path = plist_path().unwrap();

    // SAFETY: single-threaded harness; restore the saved value before any assertion can unwind.
    unsafe {
        match prev_override {
            Some(value) => std::env::set_var("MOADIM_HOME_OVERRIDE", value),
            None => std::env::remove_var("MOADIM_HOME_OVERRIDE"),
        }
    }

    assert_eq!(
        path,
        base.join("Library/LaunchAgents/io.moadim.daemon.plist"),
        "plist_path() must land under MOADIM_HOME_OVERRIDE, not the real home"
    );
    if let Some(real_home) = dirs::home_dir() {
        assert!(
            !path.starts_with(real_home),
            "plist_path() must not resolve under the real home when the override is set"
        );
    }
}

// systemd unit + loginctl/linger coverage (Linux backend) lives in `mod_linux_tests.rs`.

#[cfg(any(target_os = "macos", target_os = "linux"))]
#[test]
fn run_succeeds_for_a_zero_exit_command() {
    super::common::run("true", &[]).unwrap();
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
#[test]
fn run_errors_on_nonzero_exit() {
    // The `!status.success()` bail arm: a command that exits non-zero maps to an error.
    assert!(super::common::run("false", &[]).is_err());
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
#[test]
fn run_errors_when_program_is_missing() {
    // The spawn-failure `map_err` arm: an absent binary cannot be launched.
    assert!(super::common::run("moadim-no-such-binary-zzzqq", &[]).is_err());
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
#[test]
fn moadim_exe_errors_when_current_exe_resolution_fails() {
    // The `map_err` arm: `std::env::current_exe()` failing is otherwise unreachable in a test, so
    // this exercises it via the `MOADIM_CURRENT_EXE_FAIL_FOR_TEST` seam (utils::process).
    let env = crate::utils::process::CURRENT_EXE_FAIL_ENV;
    let prev = std::env::var_os(env);
    // SAFETY: tests run single-threaded (RUST_TEST_THREADS=1); the var is restored below.
    unsafe {
        std::env::set_var(env, "1");
    }

    let result = super::common::moadim_exe();

    // SAFETY: single-threaded harness; restore the saved value before any assertion can unwind.
    unsafe {
        match prev {
            Some(value) => std::env::set_var(env, value),
            None => std::env::remove_var(env),
        }
    }

    assert!(result.is_err());
}

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
