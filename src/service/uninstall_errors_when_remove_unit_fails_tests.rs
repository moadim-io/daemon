
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
    // SAFETY: single-threaded test execution.
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
    assert!(!is_installed().unwrap(), "not installed before install()");
    install().unwrap();
    assert!(unit.exists(), "install writes the systemd unit file");
    assert!(is_installed().unwrap(), "installed after install()");

    uninstall().unwrap();
    assert!(!unit.exists(), "uninstall removes the unit file");
    assert!(!is_installed().unwrap(), "not installed after uninstall()");
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

#[allow(
    clippy::missing_docs_in_private_items,
    reason = "split-out module keeps the file under the linecheck limit"
)]
#[path = "loginctl_bin_never_resolves_to_real_loginctl_in_test_builds_tests.rs"]
mod loginctl_bin_never_resolves_to_real_loginctl_in_test_builds_tests;
