
#[test]
fn svc_trigger_scheduled_missing_routine_not_found() {
    let _home = TempHome::set();
    assert!(matches!(
        svc_trigger_scheduled(&new_store(), "nope"),
        Err(AppError::NotFound)
    ));
}

/// Drive `svc_trigger` with a stub `tmux` reporting `live_sessions` other-routine sessions and
/// `cap_env` as `MOADIM_MAX_CONCURRENT_RUNS`, returning whether the fire was skipped (i.e. whether
/// a `skip.log` was written for it) — shared by the two cap scenario tests below.
fn trigger_under_concurrency_cap(unique: &str, live_sessions: u32, cap_env: Option<&str>) -> bool {
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt as _;

    let _home = TempHome::set();

    let agent_name = format!("svc-trigger-cap-agent-{unique}");
    std::fs::create_dir_all(crate::paths::agents_dir()).unwrap();
    let cfg = crate::paths::agent_toml_path(&agent_name);
    std::fs::write(&cfg, "command = \"true\"\nargs = []\n").unwrap();

    let title = format!("Svc Trigger Concurrency Cap {unique}");
    let dir = std::env::temp_dir().join(format!("moadim-svc-cap-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let stub = dir.join("tmux");
    let sessions: String = (0..live_sessions).fold(String::new(), |mut acc, i| {
        use std::fmt::Write as _;
        let _ = writeln!(acc, "moadim-other-173000000{i}_1");
        acc
    });
    std::fs::write(&stub, format!("#!/bin/sh\nprintf '{sessions}'\nexit 0\n")).unwrap();
    #[cfg(unix)]
    std::fs::set_permissions(&stub, std::fs::Permissions::from_mode(0o755)).unwrap();

    let id = format!("trig-cap-{unique}");
    let store = new_store();
    let mut routine = make_routine(&id, &title, 1, 1);
    routine.agent = agent_name;
    crate::routine_storage::write_routine(&routine).unwrap();
    store.lock().unwrap().insert(id.clone(), routine);

    let prev_tmux = std::env::var_os("MOADIM_TMUX_BIN");
    let prev_cap = std::env::var_os("MOADIM_MAX_CONCURRENT_RUNS");
    // SAFETY: tests in this crate run single-threaded (RUST_TEST_THREADS=1).
    unsafe {
        std::env::set_var("MOADIM_TMUX_BIN", &stub);
        match cap_env {
            Some(value) => std::env::set_var("MOADIM_MAX_CONCURRENT_RUNS", value),
            None => std::env::remove_var("MOADIM_MAX_CONCURRENT_RUNS"),
        }
    }

    let triggered = svc_trigger(&store, &id).unwrap();
    // The trigger still records its own timestamp and returns Ok regardless of whether the
    // launch itself was skipped — the same non-fatal shape as the overlap guard's skip in
    // `service_overlap_guard_tests.rs`.
    assert!(triggered.last_manual_trigger_at.is_some());
    let skipped = crate::paths::routine_skip_log_path(&crate::routines::slugify(&title)).exists();

    // SAFETY: single-threaded harness; restore the saved overrides.
    unsafe {
        match prev_tmux {
            Some(value) => std::env::set_var("MOADIM_TMUX_BIN", value),
            None => std::env::remove_var("MOADIM_TMUX_BIN"),
        }
        match prev_cap {
            Some(value) => std::env::set_var("MOADIM_MAX_CONCURRENT_RUNS", value),
            None => std::env::remove_var("MOADIM_MAX_CONCURRENT_RUNS"),
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
    skipped
}

#[allow(
    clippy::missing_docs_in_private_items,
    reason = "split-out module keeps the file under the linecheck limit"
)]
#[path = "svc_trigger_scheduled_skips_duplicate_fire_in_same_minute_tests.rs"]
mod svc_trigger_scheduled_skips_duplicate_fire_in_same_minute_tests;

#[allow(
    clippy::missing_docs_in_private_items,
    reason = "split-out module keeps the file under the linecheck limit"
)]
#[path = "svc_trigger_skips_spawn_when_the_global_concurrency_cap_is_reached_tests.rs"]
mod svc_trigger_skips_spawn_when_the_global_concurrency_cap_is_reached_tests;
