#![allow(
    clippy::wildcard_imports,
    clippy::too_many_arguments,
    clippy::needless_pass_by_value,
    dead_code,
    reason = "split-out module preserves existing code while keeping files under the linecheck limit"
)]
#[cfg(target_os = "macos")]
use super::*;

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
    // SAFETY: single-threaded test execution.
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
    // SAFETY: single-threaded test execution.
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
    // SAFETY: single-threaded test execution.
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
    assert!(!is_installed().unwrap(), "not installed before install()");
    install().unwrap();
    assert!(plist.exists(), "install writes the LaunchAgent plist");
    assert!(is_installed().unwrap(), "installed after install()");

    uninstall().unwrap();
    assert!(!plist.exists(), "uninstall removes the plist");
    assert!(!is_installed().unwrap(), "not installed after uninstall()");
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
