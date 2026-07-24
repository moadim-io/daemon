#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use super::*;

fn touch_dir(parent: &std::path::Path, name: &str) {
    std::fs::create_dir_all(parent.join(name)).unwrap();
}

#[test]
fn kill_sessions_for_slug_kills_only_live_matching_sessions() {
    // #333: deleting a routine must not leave its in-flight workbench session running.
    let base = std::env::temp_dir().join("moadim-cleanup-kill-slug-test");
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(&base).unwrap();

    touch_dir(&base, "deleted-100"); // matching slug, live      -> killed
    touch_dir(&base, "deleted-200"); // matching slug, dead      -> left alone (already gone)
    touch_dir(&base, "other-100"); // different slug, live       -> untouched
    touch_dir(&base, "notawb"); // no timestamp, ignored
    std::fs::write(base.join("deleted-stray"), b"x").unwrap(); // a file, not a dir -> ignored

    let alive = |session: &str| session == "moadim-deleted-100" || session == "moadim-other-100";
    let killed = std::cell::RefCell::new(Vec::new());
    let kill = |session: &str| killed.borrow_mut().push(session.to_string());

    let count = kill_sessions_for_slug(&base, "deleted", &alive, &kill);

    assert_eq!(count, 1);
    assert_eq!(killed.into_inner(), vec!["moadim-deleted-100".to_string()]);

    std::fs::remove_dir_all(&base).unwrap();
}

#[test]
fn kill_sessions_for_slug_returns_zero_for_a_missing_dir() {
    let missing =
        std::env::temp_dir().join(format!("moadim-kill-slug-missing-{}", uuid::Uuid::new_v4()));
    let _ = std::fs::remove_dir_all(&missing);
    let dead = |_session: &str| false;
    assert_eq!(
        kill_sessions_for_slug(&missing, "anything", &dead, &|_| {}),
        0
    );
}

#[test]
fn kill_sessions_for_deleted_routine_kills_the_live_workbench_session() {
    let home = std::env::temp_dir().join(format!(
        "moadim-cleanup-kill-deleted-{}",
        uuid::Uuid::new_v4()
    ));
    let prev_home = std::env::var_os("MOADIM_HOME_OVERRIDE");
    let prev_tmux = std::env::var_os("MOADIM_TMUX_BIN");
    // SAFETY: tests in this crate run single-threaded (RUST_TEST_THREADS=1); restored below.
    unsafe {
        std::env::set_var("MOADIM_HOME_OVERRIDE", &home);
        std::env::set_var("MOADIM_TMUX_BIN", "/usr/bin/true");
    }

    let workbenches = crate::paths::workbenches_dir();
    std::fs::create_dir_all(&workbenches).unwrap();
    std::fs::create_dir_all(workbenches.join("deleted-routine-1")).unwrap();
    std::fs::create_dir_all(workbenches.join("other-routine-1")).unwrap();

    let killed = kill_sessions_for_deleted_routine("deleted-routine");
    assert_eq!(
        killed, 1,
        "only the deleted routine's live session is killed"
    );
    // The workbench directory itself is left in place; only the session is force-killed.
    assert!(workbenches.join("deleted-routine-1").exists());
    assert!(workbenches.join("other-routine-1").exists());

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
fn kill_matching_sessions_with_an_always_true_predicate_kills_every_live_session() {
    // #320: the shutdown drain has no single slug to filter on — it must kill every live routine
    // session regardless of which routine spawned it.
    let base = std::env::temp_dir().join("moadim-cleanup-kill-all-test");
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(&base).unwrap();

    touch_dir(&base, "alpha-100"); // live -> killed
    touch_dir(&base, "beta-200"); // live, different slug -> killed too
    touch_dir(&base, "gamma-300"); // already dead -> left alone
    touch_dir(&base, "notawb"); // no timestamp, ignored
    std::fs::write(base.join("stray-file"), b"x").unwrap(); // a file, not a dir -> ignored

    let alive = |session: &str| session == "moadim-alpha-100" || session == "moadim-beta-200";
    let killed = std::cell::RefCell::new(Vec::new());
    let kill = |session: &str| killed.borrow_mut().push(session.to_string());

    let count = kill_matching_sessions(&base, &|_slug| true, &alive, &kill);

    assert_eq!(count, 2);
    let mut names = killed.into_inner();
    names.sort();
    assert_eq!(
        names,
        vec![
            "moadim-alpha-100".to_string(),
            "moadim-beta-200".to_string()
        ]
    );

    std::fs::remove_dir_all(&base).unwrap();
}

#[test]
fn kill_all_routine_sessions_kills_every_live_session_regardless_of_slug() {
    // #320: `moadim stop` must drain every in-flight routine session, not just one routine's.
    let home = std::env::temp_dir().join(format!(
        "moadim-cleanup-kill-all-routine-{}",
        uuid::Uuid::new_v4()
    ));
    let prev_home = std::env::var_os("MOADIM_HOME_OVERRIDE");
    let prev_tmux = std::env::var_os("MOADIM_TMUX_BIN");
    // SAFETY: tests in this crate run single-threaded (RUST_TEST_THREADS=1); restored below.
    unsafe {
        std::env::set_var("MOADIM_HOME_OVERRIDE", &home);
        std::env::set_var("MOADIM_TMUX_BIN", "/usr/bin/true");
    }

    let workbenches = crate::paths::workbenches_dir();
    std::fs::create_dir_all(&workbenches).unwrap();
    std::fs::create_dir_all(workbenches.join("routine-one-1")).unwrap();
    std::fs::create_dir_all(workbenches.join("routine-two-1")).unwrap();

    let killed = kill_all_routine_sessions();
    assert_eq!(
        killed, 2,
        "every live routine session is killed, not just one slug's"
    );
    // Workbench directories are left in place; only the sessions are force-killed.
    assert!(workbenches.join("routine-one-1").exists());
    assert!(workbenches.join("routine-two-1").exists());

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
