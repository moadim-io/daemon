
#[test]
fn tmux_session_count_counts_every_session_starting_with_the_prefix() {
    // The global concurrency cap (#335) needs a total *count* across every routine's sessions, not
    // just any-vs-none, so this stubs `tmux list-sessions` with a mix of matching and unrelated
    // session names and checks the count reflects only those starting with the given prefix.
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt as _;

    let dir = std::env::temp_dir().join(format!("moadim-tmux-count-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let stub = dir.join("tmux");
    std::fs::write(
        &stub,
        "#!/bin/sh\nprintf 'moadim-foo-100_1\\nmoadim-bar-200_2\\nunrelated-session\\n'\nexit 0\n",
    )
    .unwrap();
    #[cfg(unix)]
    std::fs::set_permissions(&stub, std::fs::Permissions::from_mode(0o755)).unwrap();

    let previous = std::env::var_os("MOADIM_TMUX_BIN");
    // SAFETY: tests in this crate run single-threaded (RUST_TEST_THREADS=1).
    unsafe { std::env::set_var("MOADIM_TMUX_BIN", &stub) };

    assert_eq!(
        super::session::tmux_session_count("moadim-"),
        2,
        "both moadim-prefixed sessions count, regardless of which routine/slug spawned them"
    );
    assert_eq!(super::session::tmux_session_count("moadim-nonexistent-"), 0);

    // SAFETY: single-threaded harness; restore the saved override.
    unsafe {
        match previous {
            Some(value) => std::env::set_var("MOADIM_TMUX_BIN", value),
            None => std::env::remove_var("MOADIM_TMUX_BIN"),
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn tmux_session_count_is_zero_when_tmux_bin_missing_or_fails() {
    // No live server / no tmux at all must read as zero live sessions, not an error — mirrors
    // `tmux_session_alive`'s and `tmux_session_prefix_alive`'s "no tmux" stance.
    let previous = std::env::var_os("MOADIM_TMUX_BIN");
    // SAFETY: single-threaded harness.
    unsafe { std::env::set_var("MOADIM_TMUX_BIN", "/usr/bin/false") };
    assert_eq!(super::session::tmux_session_count("moadim-"), 0);

    // SAFETY: single-threaded harness; restore the saved override.
    unsafe {
        match previous {
            Some(value) => std::env::set_var("MOADIM_TMUX_BIN", value),
            None => std::env::remove_var("MOADIM_TMUX_BIN"),
        }
    }
}

#[test]
fn note_forced_kill_is_silent_when_log_cannot_be_opened() {
    // The workbench directory does not exist, so opening `agent.log` (append+create) fails because
    // its parent is absent — exercising the `if let Ok` fall-through (the best-effort Err branch).
    let missing = std::env::temp_dir().join(format!("moadim-nfk-missing-{}", uuid::Uuid::new_v4()));
    let _ = std::fs::remove_dir_all(&missing);
    super::session::note_forced_kill(&missing);
    // Nothing is created when the open fails.
    assert!(!missing.exists());
}

#[test]
fn cleanup_expired_workbenches_kills_a_live_expired_session() {
    // With the tmux seam pointed at a 0-exit stub every session reads as *alive*, so the watchdog's
    // `alive && is_expired(.., max_runtime_for(slug))` actually evaluates the `max_runtime_for`
    // closure (the existing test's absent-tmux path short-circuits before it). The expired workbench
    // is force-killed and reaped.
    let home = std::env::temp_dir().join(format!("moadim-cleanup-live-{}", uuid::Uuid::new_v4()));
    let prev_home = std::env::var_os("MOADIM_HOME_OVERRIDE");
    let prev_tmux = std::env::var_os("MOADIM_TMUX_BIN");
    // SAFETY: tests in this crate run single-threaded (RUST_TEST_THREADS=1); restored below.
    unsafe {
        std::env::set_var("MOADIM_HOME_OVERRIDE", &home);
        std::env::set_var("MOADIM_TMUX_BIN", "/usr/bin/true");
    }

    let workbenches = crate::paths::workbenches_dir();
    std::fs::create_dir_all(&workbenches).unwrap();
    // Timestamp 1 → far past any default max-runtime ceiling → expired, so the live session is killed.
    std::fs::create_dir_all(workbenches.join("alive-1")).unwrap();

    let store = super::super::model::new_store();
    let stats = cleanup_expired_workbenches(&store);
    assert!(
        stats.removed >= 1,
        "the live, expired workbench is killed and reaped"
    );
    assert!(!workbenches.join("alive-1").exists());

    // SAFETY: single-threaded harness; restore the saved overrides.
    unsafe {
        match prev_home {
            Some(value) => std::env::set_var("MOADIM_HOME_OVERRIDE", value),
            None => std::env::remove_var("MOADIM_HOME_OVERRIDE"),
        }
        match prev_tmux {
            Some(value) => std::env::set_var("MOADIM_TMUX_BIN", value),
            None => std::env::remove_var("MOADIM_TMUX_BIN"),
        }
    }
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn tmux_bin_honors_the_override_env_var() {
    // With `MOADIM_TMUX_BIN` set, `tmux_bin` returns it verbatim, ahead of the cfg(test) fallback —
    // the seam that lets a test point probes/kills at a controlled stub.
    let previous = std::env::var_os("MOADIM_TMUX_BIN");
    // SAFETY: tests in this crate run single-threaded (RUST_TEST_THREADS=1); restored below.
    unsafe {
        std::env::set_var("MOADIM_TMUX_BIN", "/tmp/moadim-test-tmux-override");
    }

    assert_eq!(super::session::tmux_bin(), "/tmp/moadim-test-tmux-override");

    // SAFETY: single-threaded harness; restore the saved override.
    unsafe {
        match previous {
            Some(value) => std::env::set_var("MOADIM_TMUX_BIN", value),
            None => std::env::remove_var("MOADIM_TMUX_BIN"),
        }
    }
}
