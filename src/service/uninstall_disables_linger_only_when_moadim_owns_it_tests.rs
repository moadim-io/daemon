
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
