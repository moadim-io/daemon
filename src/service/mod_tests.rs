#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use super::*;

#[cfg(target_os = "macos")]
#[test]
fn plist_carries_label_program_args_and_supervision_keys() {
    let plist = render_plist(
        std::path::Path::new("/opt/moadim/bin/moadim"),
        std::path::Path::new("/Users/u/.config/moadim/daemon.log"),
        std::path::Path::new("/Users/u"),
    );
    assert!(plist.contains("<string>io.moadim.daemon</string>"));
    assert!(plist.contains("<string>/opt/moadim/bin/moadim</string>"));
    assert!(plist.contains("<string>--interactive</string>"));
    assert!(plist.contains("<key>RunAtLoad</key>"));
    assert!(plist.contains("<key>KeepAlive</key>"));
    assert!(plist.contains("/Users/u/.config/moadim/daemon.log"));
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
    assert!(unit.contains("Restart=always"));
}

#[cfg(target_os = "linux")]
#[test]
fn unit_path_is_under_systemd_user() {
    let path = unit_path().unwrap();
    assert!(path.ends_with("systemd/user/moadim.service"));
}

#[cfg(target_os = "linux")]
#[test]
fn unit_path_errors_when_config_dir_is_unknown() {
    // Covers the `ok_or_else` error arm of `unit_path_from_config_dir` (config directory
    // undeterminable).
    assert!(unit_path_from_config_dir(None).is_err());
    // And the happy path resolves under the given config directory.
    let path =
        unit_path_from_config_dir(Some(std::path::PathBuf::from("/home/u/.config"))).unwrap();
    assert!(path.ends_with("systemd/user/moadim.service"));
}

#[cfg(target_os = "linux")]
#[test]
fn systemctl_bin_never_resolves_to_real_systemctl_in_test_builds() {
    // Structural guard mirroring `launchctl_bin_never_resolves_to_real_launchctl_in_test_builds`:
    // in a test build, with no `MOADIM_SYSTEMCTL_BIN` shim configured, `systemctl_bin()` must never
    // fall back to the real `systemctl`, so a test that forgets to isolate it cannot mutate the
    // developer's live systemd session. The resolved path must also not exist, so the eventual
    // spawn fails harmlessly.
    let previous = std::env::var_os("MOADIM_SYSTEMCTL_BIN");
    // SAFETY: tests run single-threaded (RUST_TEST_THREADS=1); restored below.
    unsafe {
        std::env::remove_var("MOADIM_SYSTEMCTL_BIN");
    }
    let bin = systemctl_bin();
    // SAFETY: single-threaded harness; restore the saved value if any.
    unsafe {
        match previous {
            Some(value) => std::env::set_var("MOADIM_SYSTEMCTL_BIN", value),
            None => std::env::remove_var("MOADIM_SYSTEMCTL_BIN"),
        }
    }
    assert_ne!(
        bin, "systemctl",
        "test build must not fall back to the real systemctl"
    );
    assert!(
        !std::path::Path::new(&bin).exists(),
        "the test-build systemctl guard path must not exist so the spawn fails: {bin}"
    );
}

#[cfg(target_os = "linux")]
#[test]
fn write_unit_skips_dir_creation_when_paths_have_no_parent() {
    // Exercises the `None` arm of the defensive `if let Some(dir) = unit.parent()` guard: a
    // parent-less path ("") skips create_dir_all. The trailing write then fails, which is
    // expected — only the no-parent branch needs exercising.
    let no_parent = std::path::Path::new("");
    assert!(write_unit(no_parent, no_parent).is_err());
}

#[cfg(target_os = "linux")]
#[test]
fn write_unit_errors_when_dir_creation_blocked() {
    // Covers the `?` error branch at create_dir_all (unit parent dir): a regular file sitting
    // where the parent directory should be prevents create_dir_all.
    let base = std::env::temp_dir().join(format!("moadim-wu-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&base).unwrap();
    std::fs::write(base.join("systemd"), "block").unwrap();
    let unit = base.join("systemd/user/moadim.service");
    assert!(write_unit(&unit, std::path::Path::new("/usr/local/bin/moadim")).is_err());
    let _ = std::fs::remove_dir_all(&base);
}

#[cfg(target_os = "linux")]
#[test]
fn install_errors_when_write_unit_fails() {
    // Covers the `?` error branch on write_unit(...) inside install(): block the systemd/user
    // directory so write_unit cannot create it.
    let base = std::env::temp_dir().join(format!("moadim-inst-wu-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&base).unwrap();
    std::fs::write(base.join("systemd"), "block").unwrap();

    let prev_xdg = std::env::var_os("XDG_CONFIG_HOME");
    // SAFETY: tests run single-threaded (RUST_TEST_THREADS=1).
    unsafe {
        std::env::set_var("XDG_CONFIG_HOME", &base);
    }
    let result = install();
    unsafe {
        match prev_xdg {
            Some(val) => std::env::set_var("XDG_CONFIG_HOME", val),
            None => std::env::remove_var("XDG_CONFIG_HOME"),
        }
    }
    assert!(result.is_err(), "install must fail when write_unit fails");
    let _ = std::fs::remove_dir_all(&base);
}

#[cfg(target_os = "linux")]
#[test]
fn install_errors_when_enable_unit_fails() {
    use std::os::unix::fs::PermissionsExt as _;
    // Covers the `?` error branch on enable_unit() inside install(): write_unit succeeds, then
    // the systemctl shim exits 1, making `daemon-reload` fail.
    let base = std::env::temp_dir().join(format!("moadim-inst-eu-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&base).unwrap();
    let shim = base.join("systemctl");
    std::fs::write(&shim, "#!/bin/sh\nexit 1\n").unwrap();
    std::fs::set_permissions(&shim, std::fs::Permissions::from_mode(0o755)).unwrap();

    let prev_xdg = std::env::var_os("XDG_CONFIG_HOME");
    let prev_systemctl = std::env::var_os("MOADIM_SYSTEMCTL_BIN");
    // SAFETY: tests run single-threaded (RUST_TEST_THREADS=1).
    unsafe {
        std::env::set_var("XDG_CONFIG_HOME", &base);
        std::env::set_var("MOADIM_SYSTEMCTL_BIN", &shim);
    }
    let result = install();
    unsafe {
        match prev_xdg {
            Some(val) => std::env::set_var("XDG_CONFIG_HOME", val),
            None => std::env::remove_var("XDG_CONFIG_HOME"),
        }
        match prev_systemctl {
            Some(val) => std::env::set_var("MOADIM_SYSTEMCTL_BIN", val),
            None => std::env::remove_var("MOADIM_SYSTEMCTL_BIN"),
        }
    }
    assert!(
        result.is_err(),
        "install must fail when systemctl daemon-reload fails"
    );
    let _ = std::fs::remove_dir_all(&base);
}

#[cfg(target_os = "linux")]
#[test]
fn uninstall_errors_when_remove_unit_fails() {
    use std::os::unix::fs::PermissionsExt as _;
    // Covers the `?` error branch on remove_file(&unit) inside uninstall(): the unit exists but
    // its directory is read-only, preventing deletion.
    let base = std::env::temp_dir().join(format!("moadim-uninst-rm-{}", uuid::Uuid::new_v4()));
    let unit_dir = base.join("systemd/user");
    std::fs::create_dir_all(&unit_dir).unwrap();
    let unit = unit_dir.join("moadim.service");
    std::fs::write(&unit, "unit content").unwrap();
    std::fs::set_permissions(&unit_dir, std::fs::Permissions::from_mode(0o555)).unwrap();

    let prev_xdg = std::env::var_os("XDG_CONFIG_HOME");
    let prev_systemctl = std::env::var_os("MOADIM_SYSTEMCTL_BIN");
    // SAFETY: tests run single-threaded (RUST_TEST_THREADS=1).
    unsafe {
        std::env::set_var("XDG_CONFIG_HOME", &base);
        // Use /bin/true so the best-effort `systemctl disable` succeeds (result is ignored anyway).
        std::env::set_var("MOADIM_SYSTEMCTL_BIN", "/bin/true");
    }
    let result = uninstall();
    // Restore write permission so the directory can be cleaned up.
    let _ = std::fs::set_permissions(&unit_dir, std::fs::Permissions::from_mode(0o755));
    unsafe {
        match prev_xdg {
            Some(val) => std::env::set_var("XDG_CONFIG_HOME", val),
            None => std::env::remove_var("XDG_CONFIG_HOME"),
        }
        match prev_systemctl {
            Some(val) => std::env::set_var("MOADIM_SYSTEMCTL_BIN", val),
            None => std::env::remove_var("MOADIM_SYSTEMCTL_BIN"),
        }
    }
    assert!(
        result.is_err(),
        "uninstall must fail when the unit file cannot be removed"
    );
    let _ = std::fs::remove_dir_all(&base);
}

#[cfg(target_os = "linux")]
#[test]
fn install_then_uninstall_round_trips_against_a_sandbox() {
    use std::os::unix::fs::PermissionsExt as _;
    // Sandbox the real install path: redirect `XDG_CONFIG_HOME` (where the unit file lives) and
    // replace `systemctl` with a no-op shim via `MOADIM_SYSTEMCTL_BIN`, so no real systemd user
    // service is ever (dis/en)abled.
    let base = std::env::temp_dir().join(format!("moadim-svc-install-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&base).unwrap();
    let shim = base.join("systemctl");
    std::fs::write(&shim, "#!/bin/sh\nexit 0\n").unwrap();
    std::fs::set_permissions(&shim, std::fs::Permissions::from_mode(0o755)).unwrap();

    let prev_xdg = std::env::var_os("XDG_CONFIG_HOME");
    let prev_systemctl = std::env::var_os("MOADIM_SYSTEMCTL_BIN");
    // SAFETY: tests run single-threaded (RUST_TEST_THREADS=1); both vars are restored below.
    unsafe {
        std::env::set_var("XDG_CONFIG_HOME", &base);
        std::env::set_var("MOADIM_SYSTEMCTL_BIN", &shim);
    }

    let unit = base.join("systemd/user/moadim.service");
    install().unwrap();
    assert!(unit.exists(), "install writes the systemd unit file");

    uninstall().unwrap();
    assert!(!unit.exists(), "uninstall removes the unit file");
    // A second uninstall exercises the not-installed branch and must not error.
    uninstall().unwrap();

    // SAFETY: single-threaded harness; restore the saved values.
    unsafe {
        match prev_xdg {
            Some(value) => std::env::set_var("XDG_CONFIG_HOME", value),
            None => std::env::remove_var("XDG_CONFIG_HOME"),
        }
        match prev_systemctl {
            Some(value) => std::env::set_var("MOADIM_SYSTEMCTL_BIN", value),
            None => std::env::remove_var("MOADIM_SYSTEMCTL_BIN"),
        }
    }
    let _ = std::fs::remove_dir_all(&base);
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
    assert!(write_plist(no_parent, no_parent, no_parent, no_parent).is_err());
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
        &base
    )
    .is_err());
    let _ = std::fs::remove_dir_all(&base);
}

#[cfg(target_os = "macos")]
#[test]
fn install_errors_when_write_plist_fails() {
    // Covers the `?` error branch on write_plist(...) inside install() (L120).
    // Block the LaunchAgents directory so write_plist cannot create it.
    let base = std::env::temp_dir().join(format!("moadim-inst-wp-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(base.join("Library")).unwrap();
    std::fs::write(base.join("Library/LaunchAgents"), "block").unwrap();

    let prev_home = std::env::var_os("HOME");
    let prev_override = std::env::var_os("MOADIM_HOME_OVERRIDE");
    // SAFETY: tests run single-threaded (RUST_TEST_THREADS=1).
    unsafe {
        std::env::set_var("HOME", &base);
        std::env::set_var("MOADIM_HOME_OVERRIDE", &base);
    }
    let result = install();
    unsafe {
        match prev_home {
            Some(val) => std::env::set_var("HOME", val),
            None => std::env::remove_var("HOME"),
        }
        match prev_override {
            Some(val) => std::env::set_var("MOADIM_HOME_OVERRIDE", val),
            None => std::env::remove_var("MOADIM_HOME_OVERRIDE"),
        }
    }
    assert!(result.is_err(), "install must fail when write_plist fails");
    let _ = std::fs::remove_dir_all(&base);
}

#[cfg(target_os = "macos")]
#[test]
fn install_errors_when_reload_agent_fails() {
    use std::os::unix::fs::PermissionsExt as _;
    // Covers the `?` error branch on reload_agent(&plist) inside install() (L121).
    // write_plist succeeds; then the launchctl shim exits 1, making load fail.
    let base = std::env::temp_dir().join(format!("moadim-inst-ra-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&base).unwrap();
    let shim = base.join("launchctl");
    std::fs::write(&shim, "#!/bin/sh\nexit 1\n").unwrap();
    std::fs::set_permissions(&shim, std::fs::Permissions::from_mode(0o755)).unwrap();

    let prev_home = std::env::var_os("HOME");
    let prev_override = std::env::var_os("MOADIM_HOME_OVERRIDE");
    let prev_launchctl = std::env::var_os("MOADIM_LAUNCHCTL_BIN");
    // SAFETY: tests run single-threaded (RUST_TEST_THREADS=1).
    unsafe {
        std::env::set_var("HOME", &base);
        std::env::set_var("MOADIM_HOME_OVERRIDE", &base);
        std::env::set_var("MOADIM_LAUNCHCTL_BIN", &shim);
    }
    let result = install();
    unsafe {
        match prev_home {
            Some(val) => std::env::set_var("HOME", val),
            None => std::env::remove_var("HOME"),
        }
        match prev_override {
            Some(val) => std::env::set_var("MOADIM_HOME_OVERRIDE", val),
            None => std::env::remove_var("MOADIM_HOME_OVERRIDE"),
        }
        match prev_launchctl {
            Some(val) => std::env::set_var("MOADIM_LAUNCHCTL_BIN", val),
            None => std::env::remove_var("MOADIM_LAUNCHCTL_BIN"),
        }
    }
    assert!(
        result.is_err(),
        "install must fail when launchctl load fails"
    );
    let _ = std::fs::remove_dir_all(&base);
}

#[cfg(target_os = "macos")]
#[test]
fn uninstall_errors_when_remove_plist_fails() {
    use std::os::unix::fs::PermissionsExt as _;
    // Covers the `?` error branch on remove_file(&plist) inside uninstall() (L151).
    // The plist exists but the LaunchAgents directory is read-only, preventing deletion.
    let base = std::env::temp_dir().join(format!("moadim-uninst-rm-{}", uuid::Uuid::new_v4()));
    let launch_agents = base.join("Library/LaunchAgents");
    std::fs::create_dir_all(&launch_agents).unwrap();
    let plist = launch_agents.join("io.moadim.daemon.plist");
    std::fs::write(&plist, "plist content").unwrap();
    // Lock the directory so remove_file fails with "Permission denied".
    std::fs::set_permissions(&launch_agents, std::fs::Permissions::from_mode(0o555)).unwrap();

    let prev_home = std::env::var_os("HOME");
    let prev_override = std::env::var_os("MOADIM_HOME_OVERRIDE");
    let prev_launchctl = std::env::var_os("MOADIM_LAUNCHCTL_BIN");
    // SAFETY: tests run single-threaded (RUST_TEST_THREADS=1).
    unsafe {
        std::env::set_var("HOME", &base);
        std::env::set_var("MOADIM_HOME_OVERRIDE", &base);
        // Use /bin/true so the best-effort `launchctl unload` succeeds (result is ignored anyway).
        std::env::set_var("MOADIM_LAUNCHCTL_BIN", "/bin/true");
    }
    let result = uninstall();
    // Restore write permission so the directory can be cleaned up.
    let _ = std::fs::set_permissions(&launch_agents, std::fs::Permissions::from_mode(0o755));
    unsafe {
        match prev_home {
            Some(val) => std::env::set_var("HOME", val),
            None => std::env::remove_var("HOME"),
        }
        match prev_override {
            Some(val) => std::env::set_var("MOADIM_HOME_OVERRIDE", val),
            None => std::env::remove_var("MOADIM_HOME_OVERRIDE"),
        }
        match prev_launchctl {
            Some(val) => std::env::set_var("MOADIM_LAUNCHCTL_BIN", val),
            None => std::env::remove_var("MOADIM_LAUNCHCTL_BIN"),
        }
    }
    assert!(
        result.is_err(),
        "uninstall must fail when plist file cannot be removed"
    );
    let _ = std::fs::remove_dir_all(&base);
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
