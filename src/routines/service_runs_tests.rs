#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use super::*;

use crate::routines::model::RunStatus;
use crate::routines::new_store;

struct TempHome(std::path::PathBuf);

impl TempHome {
    fn set() -> Self {
        let dir = std::env::temp_dir().join(format!("moadim-runstest-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("create temp home");
        // SAFETY: single-threaded test execution.
        unsafe {
            std::env::set_var("MOADIM_HOME_OVERRIDE", &dir);
        }
        Self(dir)
    }
}

impl Drop for TempHome {
    fn drop(&mut self) {
        // SAFETY: single-threaded test execution.
        unsafe {
            std::env::remove_var("MOADIM_HOME_OVERRIDE");
        }
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Point `MOADIM_TMUX_BIN` at a shim for the duration of `body`, restoring the previous value
/// (or clearing it) afterwards. `/usr/bin/true` always exits `0`, so `tmux has-session` reads as
/// "alive" no matter the session name; leaving it unset (test default) makes every session read
/// as "not alive" (see `session::tmux_bin`'s `cfg(test)` fallback to a nonexistent path).
fn with_tmux_alive(body: impl FnOnce()) {
    let previous = std::env::var_os("MOADIM_TMUX_BIN");
    // SAFETY: single-threaded test execution.
    unsafe {
        std::env::set_var("MOADIM_TMUX_BIN", "/usr/bin/true");
    }
    body();
    // SAFETY: single-threaded test execution.
    unsafe {
        match previous {
            Some(value) => std::env::set_var("MOADIM_TMUX_BIN", value),
            None => std::env::remove_var("MOADIM_TMUX_BIN"),
        }
    }
}

fn make_routine(id: &str, title: &str) -> Routine {
    Routine {
        model: None,
        id: id.to_string(),
        schedule: "@daily".to_string(),
        schedules: vec![],
        title: title.to_string(),
        agent: "claude".to_string(),
        prompt: "do the thing".to_string(),
        goal: None,
        repositories: vec![],
        machines: vec![crate::machine::current_machine()],
        enabled: true,
        disabled_reason: None,
        source: "managed".to_string(),
        created_at: 1,
        updated_at: 1,
        last_manual_trigger_at: None,
        last_scheduled_trigger_at: None,
        snoozed_until: None,
        skip_runs: None,
        power_saving: false,
        power_saving_exempt: false,
        tags: vec![],
        ttl_secs: None,
        max_runtime_secs: None,
        env: std::collections::HashMap::new(),
        auto_disabled_reason: None,
        consecutive_failures: 0,
        failure_threshold: None,
        notifications: Default::default(),
    }
}

#[test]
fn svc_list_runs_missing_routine_not_found() {
    let _home = TempHome::set();
    assert!(matches!(
        svc_list_runs(&new_store(), "nope"),
        Err(AppError::NotFound)
    ));
}

#[test]
fn svc_list_runs_empty_when_workbenches_dir_absent() {
    let _home = TempHome::set();
    let store = new_store();
    store
        .lock()
        .unwrap()
        .insert("id".into(), make_routine("id", "Runs No Workbenches ZZQ"));

    assert!(!crate::paths::workbenches_dir().exists());
    assert_eq!(svc_list_runs(&store, "id").unwrap(), vec![]);
}

#[test]
fn svc_list_runs_skips_foreign_and_unparseable_workbenches() {
    let _home = TempHome::set();
    let title = "Runs Mixed ZZQ";
    let slug = slugify(title);
    let store = new_store();
    store
        .lock()
        .unwrap()
        .insert("id".into(), make_routine("id", title));

    let workbenches = crate::paths::workbenches_dir();
    std::fs::create_dir_all(&workbenches).unwrap();
    std::fs::create_dir_all(workbenches.join("not-a-workbench-name")).unwrap();
    std::fs::create_dir_all(workbenches.join("some-other-routine-9999")).unwrap();
    std::fs::create_dir_all(workbenches.join(format!("{slug}-1000"))).unwrap();

    let runs = svc_list_runs(&store, "id").unwrap();
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].workbench, format!("{slug}-1000"));
}

#[test]
fn svc_list_runs_derives_status_newest_first() {
    let _home = TempHome::set();
    let title = "Runs Status ZZQ";
    let slug = slugify(title);
    let store = new_store();
    store
        .lock()
        .unwrap()
        .insert("id".into(), make_routine("id", title));

    let workbenches = crate::paths::workbenches_dir();
    let success = workbenches.join(format!("{slug}-1000"));
    let failed = workbenches.join(format!("{slug}-2000"));
    let unknown = workbenches.join(format!("{slug}-3000"));
    for dir in [&success, &failed, &unknown] {
        std::fs::create_dir_all(dir).unwrap();
    }
    std::fs::write(success.join("exit_code"), "0").unwrap();
    std::fs::write(failed.join("exit_code"), "1").unwrap();
    // `unknown` gets no exit_code file and (with no MOADIM_TMUX_BIN shim) no live session.

    let runs = svc_list_runs(&store, "id").unwrap();
    // Newest (highest trigger timestamp) first.
    assert_eq!(
        runs.iter().map(|run| run.started_at).collect::<Vec<_>>(),
        vec![3000, 2000, 1000]
    );
    assert_eq!(runs[0].status, RunStatus::Unknown);
    assert_eq!(runs[0].exit_code, None);
    assert_eq!(runs[1].status, RunStatus::Failed);
    assert_eq!(runs[1].exit_code, Some(1));
    assert_eq!(runs[2].status, RunStatus::Success);
    assert_eq!(runs[2].exit_code, Some(0));
    assert!(runs[2].finished_at.is_some());
    assert!(runs[0].finished_at.is_none());
}
include!("svc_list_runs_reports_running_when_session_alive_and_no_exit_code_tests.rs");
