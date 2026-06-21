#![allow(clippy::missing_docs_in_private_items)]

use super::*;

#[cfg(target_os = "macos")]
#[test]
fn plist_carries_label_program_args_and_supervision_keys() {
    let plist = render_plist(
        std::path::Path::new("/opt/moadim/bin/moadim"),
        std::path::Path::new("/Users/u/.config/moadim/daemon.log"),
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
}

#[cfg(target_os = "macos")]
#[test]
fn plist_escapes_xml_metacharacters_in_paths() {
    let plist = render_plist(
        std::path::Path::new("/tmp/a&b<c>"),
        std::path::Path::new("/tmp/log"),
    );
    assert!(plist.contains("/tmp/a&amp;b&lt;c&gt;"));
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

#[cfg(target_os = "linux")]
#[test]
fn unit_carries_exec_start_and_install_section() {
    let unit = render_unit(std::path::Path::new("/opt/moadim/bin/moadim"));
    assert!(unit.contains("ExecStart=/opt/moadim/bin/moadim --interactive"));
    assert!(unit.contains("[Install]"));
    assert!(unit.contains("WantedBy=default.target"));
    // Restart is failure-only, so a clean `moadim stop` (exit 0) is not resurrected by systemd
    // while a crash still auto-restarts (#444).
    assert!(unit.contains("Restart=on-failure"));
    assert!(!unit.contains("Restart=always"));
}

#[cfg(target_os = "linux")]
#[test]
fn unit_path_is_under_systemd_user() {
    let path = unit_path().unwrap();
    assert!(path.ends_with("systemd/user/moadim.service"));
}

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
    assert!(write_plist(no_parent, no_parent, no_parent).is_err());
}

#[cfg(target_os = "macos")]
#[test]
fn install_then_uninstall_round_trips_against_a_sandbox() {
    use std::os::unix::fs::PermissionsExt as _;

    // Sandbox the real install path: redirect `$HOME` (where the LaunchAgent plist lives) and
    // `MOADIM_HOME_OVERRIDE` (the daemon log path) to a temp dir, and replace `launchctl` with a
    // no-op shim via `MOADIM_LAUNCHCTL_BIN`, so no real launchd agent is ever (un)loaded.
    let base = std::env::temp_dir().join(format!("moadim-svc-install-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&base).unwrap();
    let shim = base.join("launchctl");
    std::fs::write(&shim, "#!/bin/sh\nexit 0\n").unwrap();
    std::fs::set_permissions(&shim, std::fs::Permissions::from_mode(0o755)).unwrap();

    let prev_home = std::env::var_os("HOME");
    let prev_override = std::env::var_os("MOADIM_HOME_OVERRIDE");
    let prev_launchctl = std::env::var_os("MOADIM_LAUNCHCTL_BIN");
    // SAFETY: tests run single-threaded (RUST_TEST_THREADS=1); all three vars are restored below.
    unsafe {
        std::env::set_var("HOME", &base);
        std::env::set_var("MOADIM_HOME_OVERRIDE", &base);
        std::env::set_var("MOADIM_LAUNCHCTL_BIN", &shim);
    }

    let plist = base.join("Library/LaunchAgents/io.moadim.daemon.plist");
    install().unwrap();
    assert!(plist.exists(), "install writes the LaunchAgent plist");

    uninstall().unwrap();
    assert!(!plist.exists(), "uninstall removes the plist");
    // A second uninstall exercises the not-installed branch and must not error.
    uninstall().unwrap();

    // SAFETY: single-threaded harness; restore the saved values.
    unsafe {
        match prev_home {
            Some(value) => std::env::set_var("HOME", value),
            None => std::env::remove_var("HOME"),
        }
        match prev_override {
            Some(value) => std::env::set_var("MOADIM_HOME_OVERRIDE", value),
            None => std::env::remove_var("MOADIM_HOME_OVERRIDE"),
        }
        match prev_launchctl {
            Some(value) => std::env::set_var("MOADIM_LAUNCHCTL_BIN", value),
            None => std::env::remove_var("MOADIM_LAUNCHCTL_BIN"),
        }
    }
    let _ = std::fs::remove_dir_all(&base);
}
