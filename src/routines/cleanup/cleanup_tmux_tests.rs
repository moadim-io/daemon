#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use super::*;

#[test]
fn tmux_kill_session_is_best_effort_on_missing_session() {
    // The real tmux side-effect helper. Killing a session that does not exist (or running with no
    // tmux installed) must not panic — the call is best-effort and any failure is swallowed.
    tmux_kill_session("moadim-nonexistent-watchdog-test-session");
}

#[test]
fn tmux_bin_falls_back_to_a_nonexistent_path_under_cfg_test() {
    // Without an override, the test-build fallback must NOT be the real `tmux` binary, and must
    // point at a path that does not exist, so probes/kills are harmless no-ops (#215). This mirrors
    // the crontab_bin guard (#211).
    let previous = std::env::var_os("MOADIM_TMUX_BIN");
    // SAFETY: tests in this crate run single-threaded (RUST_TEST_THREADS=1); restored below.
    unsafe {
        std::env::remove_var("MOADIM_TMUX_BIN");
    }

    let bin = super::session::tmux_bin();
    assert_ne!(bin, "tmux", "test build must not spawn the real tmux");
    assert!(
        !std::path::Path::new(&bin).exists(),
        "test fallback must point at a non-existent path: {bin}"
    );

    // SAFETY: single-threaded harness; restore the saved override.
    unsafe {
        match previous {
            Some(value) => std::env::set_var("MOADIM_TMUX_BIN", value),
            None => std::env::remove_var("MOADIM_TMUX_BIN"),
        }
    }
}

#[test]
fn tmux_session_alive_reflects_the_bin_exit_status() {
    // Point the tmux seam at real binaries that exit 0 / non-zero so the `has-session` status
    // actually resolves, exercising the `.map(|status| status.success())` branch in both directions
    // (the cfg(test) fallback path never spawns a real process, so this is the only place it runs).
    let previous = std::env::var_os("MOADIM_TMUX_BIN");
    let set = |bin: &str| {
        // SAFETY: tests in this crate run single-threaded (RUST_TEST_THREADS=1).
        unsafe { std::env::set_var("MOADIM_TMUX_BIN", bin) };
    };

    set("/usr/bin/true");
    assert!(
        super::session::tmux_session_alive("moadim-anything"),
        "a 0-exit tmux stub reads as alive"
    );
    set("/usr/bin/false");
    assert!(
        !super::session::tmux_session_alive("moadim-anything"),
        "a non-zero-exit tmux stub reads as not alive"
    );

    // SAFETY: single-threaded harness; restore the saved override.
    unsafe {
        match previous {
            Some(value) => std::env::set_var("MOADIM_TMUX_BIN", value),
            None => std::env::remove_var("MOADIM_TMUX_BIN"),
        }
    }
}

#[test]
fn tmux_session_prefix_alive_matches_any_session_starting_with_prefix() {
    // The overlap guard (#514) needs *any* live session for the routine, not one exact name, so
    // this stubs `tmux list-sessions -F ...` (unlike `tmux_session_alive`'s `has-session`, which
    // only a single hard-coded exit status can stand in for) to actually exercise the prefix match.
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt as _;

    let dir = std::env::temp_dir().join(format!("moadim-tmux-prefix-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let stub = dir.join("tmux");
    // Real `$RID` shape (`SESS="moadim-$SLUG-$RID"`, `RID="${TS}_$$"`): digits, `_`, digits.
    // `moadim-baz-qux-300_3` is a *different* routine (slug `baz-qux`) whose session name is a
    // literal string-prefix superset of slug `baz`'s own prefix — no genuine `baz` fire is listed.
    std::fs::write(
        &stub,
        "#!/bin/sh\nprintf 'moadim-other-100_1\\nmoadim-foo-200_2\\nmoadim-baz-qux-300_3\\n'\nexit 0\n",
    )
    .unwrap();
    #[cfg(unix)]
    std::fs::set_permissions(&stub, std::fs::Permissions::from_mode(0o755)).unwrap();

    let previous = std::env::var_os("MOADIM_TMUX_BIN");
    // SAFETY: tests in this crate run single-threaded (RUST_TEST_THREADS=1).
    unsafe { std::env::set_var("MOADIM_TMUX_BIN", &stub) };

    assert!(
        super::session::tmux_session_prefix_alive("moadim-foo-"),
        "a listed session starting with the prefix must read as alive"
    );
    assert!(
        !super::session::tmux_session_prefix_alive("moadim-bar-"),
        "no listed session starts with this prefix"
    );
    assert!(
        !super::session::tmux_session_prefix_alive("moadim-baz-"),
        "a different routine's session name being a string-superset of the prefix (baz-qux vs. \
         baz) must not read as baz's own fire still being alive"
    );

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
fn tmux_session_prefix_alive_false_when_tmux_bin_missing_or_fails() {
    // No live server / no tmux at all must never be mistaken for an overlapping run — mirrors
    // `tmux_session_alive`'s stance that "no tmux" means "nothing to guard against".
    let previous = std::env::var_os("MOADIM_TMUX_BIN");
    // SAFETY: single-threaded harness.
    unsafe { std::env::set_var("MOADIM_TMUX_BIN", "/usr/bin/false") };
    assert!(!super::session::tmux_session_prefix_alive("moadim-foo-"));

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
