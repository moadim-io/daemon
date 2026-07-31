#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use super::*;

/// Run `body` with `PATH` set to `value`, restoring the previous value afterwards.
///
/// The test harness is single-threaded (`RUST_TEST_THREADS=1`), so mutating the
/// process-global `PATH` and restoring it around the call is safe.
fn with_path(value: &std::path::Path, body: impl FnOnce()) {
    let saved = std::env::var_os("PATH");
    // SAFETY: single-threaded test harness; the value is restored immediately after.
    unsafe {
        std::env::set_var("PATH", value);
    }
    body();
    // SAFETY: single-threaded test execution.
    unsafe {
        match saved {
            Some(prev) => std::env::set_var("PATH", prev),
            None => std::env::remove_var("PATH"),
        }
    }
}

#[test]
fn tmux_available_in_true_when_fake_tmux_present() {
    // A temp dir containing a fake `tmux` executable resolves as available — the "present" branch
    // of the injectable detection helper.
    let dir = std::env::temp_dir().join(format!("moadim-tmux-present-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let tmux = dir.join("tmux");
    std::fs::write(&tmux, "#!/bin/sh\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(&tmux, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    assert!(tmux_available_in(&dir.to_string_lossy()));

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn tmux_available_in_false_when_dir_has_no_tmux() {
    // A temp dir without a `tmux` file resolves as missing — the "missing" branch of the helper.
    let dir = std::env::temp_dir().join(format!("moadim-tmux-missing-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();

    assert!(!tmux_available_in(&dir.to_string_lossy()));

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn tmux_available_reads_live_path_present() {
    // `tmux_available()` reads the process `PATH`; pointed at a dir with a fake tmux it returns
    // true, exercising the `is_some_and(..)` Some/true arm.
    let dir = std::env::temp_dir().join(format!("moadim-tmux-live-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let tmux = dir.join("tmux");
    std::fs::write(&tmux, "#!/bin/sh\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(&tmux, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    with_path(&dir, || assert!(tmux_available()));

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn tmux_available_false_when_path_unset() {
    // With PATH removed entirely, `std::env::var("PATH").ok()` is None and `is_some_and` short-
    // circuits to false — the missing-PATH arm.
    let saved = std::env::var_os("PATH");
    // SAFETY: single-threaded test harness; restored immediately below.
    unsafe {
        std::env::remove_var("PATH");
    }
    assert!(!tmux_available());
    // SAFETY: single-threaded test execution.
    unsafe {
        if let Some(prev) = saved {
            std::env::set_var("PATH", prev);
        }
    }
}

#[test]
fn agent_command_available_in_true_when_fake_command_present() {
    // A temp dir containing a fake agent executable resolves as available — the "present" branch
    // of the injectable detection helper.
    let dir =
        std::env::temp_dir().join(format!("moadim-agentcmd-present-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let bin = dir.join("fake-agent-cmd");
    std::fs::write(&bin, "#!/bin/sh\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    assert!(agent_command_available_in(
        &dir.to_string_lossy(),
        "fake-agent-cmd"
    ));

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn resolve_tmux_bin_from_prefers_path_over_fallbacks() {
    // tmux present on `path` -> returned immediately, fallback_dirs never consulted (Some-arm of
    // `bin_dir_in`, early return before the fallback loop).
    let dir =
        std::env::temp_dir().join(format!("moadim-resolve-tmux-path-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("tmux"), "#!/bin/sh\n").unwrap();

    let dir_str = dir.to_string_lossy().into_owned();
    let resolved = resolve_tmux_bin_from(&dir_str, &["/definitely/not/here".to_string()]);
    assert_eq!(resolved, format!("{dir_str}/tmux"));

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn agent_command_available_in_false_when_dir_has_no_command() {
    // A temp dir without the named executable resolves as missing.
    let dir =
        std::env::temp_dir().join(format!("moadim-agentcmd-missing-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();

    assert!(!agent_command_available_in(
        &dir.to_string_lossy(),
        "fake-agent-cmd"
    ));

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn resolve_tmux_bin_from_falls_back_to_first_matching_fallback_dir() {
    // Not on `path`, but present in the second fallback dir -> the `for` loop's `is_file()` Some
    // (true) arm returns from there, having skipped the first (missing) dir.
    let base =
        std::env::temp_dir().join(format!("moadim-resolve-tmux-fb-{}", uuid::Uuid::new_v4()));
    let missing = base.join("missing");
    let present = base.join("present");
    std::fs::create_dir_all(&present).unwrap();
    std::fs::write(present.join("tmux"), "#!/bin/sh\n").unwrap();

    let resolved = resolve_tmux_bin_from(
        "",
        &[
            missing.to_string_lossy().into_owned(),
            present.to_string_lossy().into_owned(),
        ],
    );
    assert_eq!(resolved, format!("{}/tmux", present.to_string_lossy()));

    let _ = std::fs::remove_dir_all(&base);
}
include!("command_bin_resolution_tests_part3.rs");
