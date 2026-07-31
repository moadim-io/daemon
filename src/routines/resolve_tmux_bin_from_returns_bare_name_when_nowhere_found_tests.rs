
#[test]
fn resolve_tmux_bin_from_returns_bare_name_when_nowhere_found() {
    // Neither `path` nor any fallback dir holds `tmux` -> the loop runs to completion and the
    // final bare `"tmux"` fallback is returned.
    let resolved = resolve_tmux_bin_from("", &["/definitely/not/here".to_string()]);
    assert_eq!(resolved, "tmux");
}

#[test]
fn tmux_fallback_dirs_are_anchored_under_home() {
    let dirs = tmux_fallback_dirs("/home/u");
    assert!(dirs.contains(&"/opt/homebrew/bin".to_string()));
    assert!(dirs.contains(&"/usr/local/bin".to_string()));
    assert!(dirs.contains(&"/home/u/.local/bin".to_string()));
}

#[test]
fn resolve_tmux_bin_reads_live_path_and_home() {
    // End-to-end live-env wrapper: with a fake tmux on PATH it resolves through the same
    // `bin_dir_in` Some-arm as `resolve_tmux_bin_from`, proving the live `PATH`/`HOME` plumbing
    // reaches it.
    let dir =
        std::env::temp_dir().join(format!("moadim-resolve-tmux-live-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("tmux"), "#!/bin/sh\n").unwrap();

    let dir_str = dir.to_string_lossy().into_owned();
    with_path(&dir, || {
        assert_eq!(resolve_tmux_bin(), format!("{dir_str}/tmux"));
    });

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn agent_command_available_reads_live_path_present() {
    // `agent_command_available()` reads the process `PATH`; pointed at a dir with the fake command
    // it returns true, exercising the `is_some_and(..)` Some/true arm.
    let dir = std::env::temp_dir().join(format!("moadim-agentcmd-live-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let bin = dir.join("fake-agent-cmd");
    std::fs::write(&bin, "#!/bin/sh\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    with_path(&dir, || assert!(agent_command_available("fake-agent-cmd")));

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn agent_command_available_false_when_path_unset() {
    // With PATH removed entirely, `std::env::var("PATH").ok()` is None and `is_some_and` short-
    // circuits to false — the missing-PATH arm.
    let saved = std::env::var_os("PATH");
    // SAFETY: single-threaded test harness; restored immediately below.
    unsafe {
        std::env::remove_var("PATH");
    }
    assert!(!agent_command_available("definitely-not-a-real-binary-xyz"));
    // SAFETY: single-threaded test execution.
    unsafe {
        if let Some(prev) = saved {
            std::env::set_var("PATH", prev);
        }
    }
}

#[allow(
    clippy::missing_docs_in_private_items,
    reason = "split-out module keeps the file under the linecheck limit"
)]
#[path = "resolve_tmux_bin_falls_back_to_root_home_when_home_unset_tests.rs"]
mod resolve_tmux_bin_falls_back_to_root_home_when_home_unset_tests;
