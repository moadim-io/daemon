#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

#[cfg(target_os = "linux")]
use super::*;

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

#[cfg(target_os = "linux")]
#[test]
fn loginctl_bin_never_resolves_to_real_loginctl_in_test_builds() {
    // Structural guard mirroring `systemctl_bin_never_resolves_to_real_systemctl_in_test_builds`:
    // with no `MOADIM_LOGINCTL_BIN` shim configured, `loginctl_bin()` must never fall back to the
    // real `loginctl`, so a test that forgets to isolate it cannot toggle the developer's live
    // lingering state.
    let previous = std::env::var_os("MOADIM_LOGINCTL_BIN");
    // SAFETY: tests run single-threaded (RUST_TEST_THREADS=1); restored below.
    unsafe {
        std::env::remove_var("MOADIM_LOGINCTL_BIN");
    }
    let bin = loginctl_bin();
    // SAFETY: single-threaded harness; restore the saved value if any.
    unsafe {
        match previous {
            Some(value) => std::env::set_var("MOADIM_LOGINCTL_BIN", value),
            None => std::env::remove_var("MOADIM_LOGINCTL_BIN"),
        }
    }
    assert_ne!(
        bin, "loginctl",
        "test build must not fall back to the real loginctl"
    );
    assert!(
        !std::path::Path::new(&bin).exists(),
        "the test-build loginctl guard path must not exist so the spawn fails: {bin}"
    );
}

#[cfg(target_os = "linux")]
#[test]
fn install_enables_linger_and_marks_ownership_when_loginctl_succeeds() {
    use std::os::unix::fs::PermissionsExt as _;
    // Covers the success arm of `enable_linger()` inside `install()` (#294): both `systemctl` and
    // `loginctl` shims exit 0, so install() writes the linger-ownership marker.
    let base = std::env::temp_dir().join(format!("moadim-linger-ok-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&base).unwrap();
    let systemctl = base.join("systemctl");
    std::fs::write(&systemctl, "#!/bin/sh\nexit 0\n").unwrap();
    std::fs::set_permissions(&systemctl, std::fs::Permissions::from_mode(0o755)).unwrap();
    let loginctl = base.join("loginctl");
    std::fs::write(&loginctl, "#!/bin/sh\nexit 0\n").unwrap();
    std::fs::set_permissions(&loginctl, std::fs::Permissions::from_mode(0o755)).unwrap();

    let prev_xdg = std::env::var_os("XDG_CONFIG_HOME");
    let prev_systemctl = std::env::var_os("MOADIM_SYSTEMCTL_BIN");
    let prev_loginctl = std::env::var_os("MOADIM_LOGINCTL_BIN");
    // SAFETY: tests run single-threaded (RUST_TEST_THREADS=1); all three restored below.
    unsafe {
        std::env::set_var("XDG_CONFIG_HOME", &base);
        std::env::set_var("MOADIM_SYSTEMCTL_BIN", &systemctl);
        std::env::set_var("MOADIM_LOGINCTL_BIN", &loginctl);
    }

    let unit = base.join("systemd/user/moadim.service");
    install().unwrap();
    let marker = linger_marker_path(&unit).unwrap();

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
        match prev_loginctl {
            Some(value) => std::env::set_var("MOADIM_LOGINCTL_BIN", value),
            None => std::env::remove_var("MOADIM_LOGINCTL_BIN"),
        }
    }
    assert!(
        marker.exists(),
        "install must record linger ownership when loginctl succeeds"
    );
    let _ = std::fs::remove_dir_all(&base);
}

#[cfg(target_os = "linux")]
#[test]
fn install_warns_without_failing_when_loginctl_fails() {
    use std::os::unix::fs::PermissionsExt as _;
    // Covers the error arm of `enable_linger()` inside `install()` (#294): `systemctl` succeeds but
    // `loginctl` exits 1 (e.g. no systemd-logind). install() must still return Ok and must not
    // write the ownership marker.
    let base = std::env::temp_dir().join(format!("moadim-linger-fail-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&base).unwrap();
    let systemctl = base.join("systemctl");
    std::fs::write(&systemctl, "#!/bin/sh\nexit 0\n").unwrap();
    std::fs::set_permissions(&systemctl, std::fs::Permissions::from_mode(0o755)).unwrap();
    let loginctl = base.join("loginctl");
    std::fs::write(&loginctl, "#!/bin/sh\nexit 1\n").unwrap();
    std::fs::set_permissions(&loginctl, std::fs::Permissions::from_mode(0o755)).unwrap();

    let prev_xdg = std::env::var_os("XDG_CONFIG_HOME");
    let prev_systemctl = std::env::var_os("MOADIM_SYSTEMCTL_BIN");
    let prev_loginctl = std::env::var_os("MOADIM_LOGINCTL_BIN");
    // SAFETY: tests run single-threaded (RUST_TEST_THREADS=1); all three restored below.
    unsafe {
        std::env::set_var("XDG_CONFIG_HOME", &base);
        std::env::set_var("MOADIM_SYSTEMCTL_BIN", &systemctl);
        std::env::set_var("MOADIM_LOGINCTL_BIN", &loginctl);
    }

    let unit = base.join("systemd/user/moadim.service");
    let result = install();
    let marker = linger_marker_path(&unit).unwrap();

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
        match prev_loginctl {
            Some(value) => std::env::set_var("MOADIM_LOGINCTL_BIN", value),
            None => std::env::remove_var("MOADIM_LOGINCTL_BIN"),
        }
    }
    assert!(
        result.is_ok(),
        "install must not fail when only linger enablement fails"
    );
    assert!(
        !marker.exists(),
        "install must not record linger ownership when loginctl fails"
    );
    let _ = std::fs::remove_dir_all(&base);
}

#[cfg(target_os = "linux")]
#[test]
fn uninstall_disables_linger_only_when_moadim_owns_it() {
    // Covers `disable_linger_if_owned()` inside `uninstall()` (#294): with the ownership marker
    // present, uninstall must invoke `loginctl disable-linger` and remove the marker; without it
    // (lingering enabled by the operator, not moadim), uninstall must leave lingering untouched.
    let base = std::env::temp_dir().join(format!("moadim-linger-uninst-{}", uuid::Uuid::new_v4()));
    let unit_dir = base.join("systemd/user");
    std::fs::create_dir_all(&unit_dir).unwrap();
    let unit = unit_dir.join("moadim.service");
    std::fs::write(&unit, "unit content").unwrap();
    let marker = linger_marker_path(&unit).unwrap();
    std::fs::write(&marker, "").unwrap();

    let prev_xdg = std::env::var_os("XDG_CONFIG_HOME");
    let prev_systemctl = std::env::var_os("MOADIM_SYSTEMCTL_BIN");
    let prev_loginctl = std::env::var_os("MOADIM_LOGINCTL_BIN");
    // SAFETY: tests run single-threaded (RUST_TEST_THREADS=1); all three restored below.
    unsafe {
        std::env::set_var("XDG_CONFIG_HOME", &base);
        std::env::set_var("MOADIM_SYSTEMCTL_BIN", "/bin/true");
        std::env::set_var("MOADIM_LOGINCTL_BIN", "/bin/true");
    }

    uninstall().unwrap();
    assert!(
        !marker.exists(),
        "uninstall must remove the ownership marker once linger is disabled"
    );

    // Re-create the unit without a marker: lingering the operator set themselves must survive.
    std::fs::write(&unit, "unit content").unwrap();
    uninstall().unwrap();
    assert!(
        !marker.exists(),
        "no marker means uninstall has nothing to clean up"
    );

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
        match prev_loginctl {
            Some(value) => std::env::set_var("MOADIM_LOGINCTL_BIN", value),
            None => std::env::remove_var("MOADIM_LOGINCTL_BIN"),
        }
    }
    let _ = std::fs::remove_dir_all(&base);
}

#[cfg(target_os = "linux")]
#[test]
fn disable_linger_if_owned_returns_when_unit_has_no_parent() {
    // Covers the `None` arm of `linger_marker_path()` reached from inside
    // `disable_linger_if_owned()`: a unit path with no parent directory (e.g. the filesystem
    // root) has nowhere to look for the ownership marker, so the function must return without
    // touching `loginctl` or panicking, instead of unwrapping `None`.
    disable_linger_if_owned(std::path::Path::new("/"));
}
