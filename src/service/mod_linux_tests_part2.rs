#![allow(
    clippy::wildcard_imports,
    clippy::too_many_arguments,
    clippy::needless_pass_by_value,
    dead_code,
    reason = "split-out module preserves existing code while keeping files under the linecheck limit"
)]
#[cfg(target_os = "linux")]
use super::super::linux::{disable_linger_if_owned, linger_marker_path, loginctl_bin};
#[cfg(target_os = "linux")]
use super::super::{install, uninstall};

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
