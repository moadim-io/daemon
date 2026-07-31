#![allow(
    clippy::wildcard_imports,
    clippy::too_many_arguments,
    clippy::needless_pass_by_value,
    dead_code,
    reason = "split-out module preserves existing code while keeping files under the linecheck limit"
)]
use super::*;

#[test]
fn resolve_tmux_bin_falls_back_to_root_home_when_home_unset() {
    // With HOME removed, `std::env::var("HOME").unwrap_or_else(|_| "/root".to_string())` takes its
    // fallback arm — mirrors `cron_path_falls_back_to_root_home_when_home_unset` for the identical
    // pattern here. `home` is computed unconditionally before the PATH/fallback-dir search, so this
    // covers the closure regardless of whether a real `tmux` is on the test machine's live PATH.
    let saved = std::env::var_os("HOME");
    // SAFETY: single-threaded test harness; restored immediately below.
    unsafe {
        std::env::remove_var("HOME");
    }

    let _ = resolve_tmux_bin();

    // SAFETY: single-threaded test execution.
    unsafe {
        match saved {
            Some(prev) => std::env::set_var("HOME", prev),
            None => std::env::remove_var("HOME"),
        }
    }
}

#[test]
fn setup_step_interpreter_returns_first_token() {
    // The built-in `claude` agent's shape: `python3 -c '...' "$WB"` — only the leading interpreter
    // name is extracted, everything else is ignored.
    assert_eq!(
        setup_step_interpreter("python3 -c 'import os' \"$WB\""),
        Some("python3")
    );
}

#[test]
fn setup_step_interpreter_none_for_blank_setup() {
    assert_eq!(setup_step_interpreter("   "), None);
    assert_eq!(setup_step_interpreter(""), None);
}

#[test]
fn setup_step_available_true_when_no_setup_step() {
    // `None` (no `setup` configured at all) is vacuously available — nothing to probe.
    assert!(setup_step_available(None));
}

#[test]
fn setup_step_available_true_when_interpreter_on_path() {
    let dir =
        std::env::temp_dir().join(format!("moadim-setupstep-present-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let bin = dir.join("fake-interpreter");
    std::fs::write(&bin, "#!/bin/sh\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    with_path(&dir, || {
        assert!(setup_step_available(Some(
            "fake-interpreter -c 'x' \"$WB\""
        )));
    });

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn setup_step_available_false_when_interpreter_missing() {
    let empty_dir =
        std::env::temp_dir().join(format!("moadim-setupstep-missing-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&empty_dir).unwrap();

    with_path(&empty_dir, || {
        assert!(!setup_step_available(Some(
            "definitely-not-a-real-binary-xyz -c 'x'"
        )));
    });

    let _ = std::fs::remove_dir_all(&empty_dir);
}

#[test]
fn bin_dir_returns_none_when_path_unset() {
    // With PATH removed entirely, `std::env::var("PATH").ok()?` short-circuits to None.
    let saved = std::env::var_os("PATH");
    // SAFETY: single-threaded test harness; restored immediately below.
    unsafe {
        std::env::remove_var("PATH");
    }
    assert!(bin_dir("definitely-not-a-real-binary-xyz").is_none());
    // SAFETY: single-threaded test execution.
    unsafe {
        if let Some(prev) = saved {
            std::env::set_var("PATH", prev);
        }
    }
}
