#![allow(clippy::missing_docs_in_private_items)]

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
        title: title.to_string(),
        agent: "claude".to_string(),
        prompt: "do the thing".to_string(),
        goal: None,
        repositories: vec![],
        machines: vec![crate::machine::current_machine()],
        enabled: true,
        source: "managed".to_string(),
        created_at: 1,
        updated_at: 1,
        last_manual_trigger_at: None,
        last_scheduled_trigger_at: None,
        snoozed_until: None,
        skip_runs: None,
        tags: vec![],
        ttl_secs: None,
        max_runtime_secs: None,
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

#[test]
fn svc_list_runs_reports_running_when_session_alive_and_no_exit_code() {
    let _home = TempHome::set();
    let title = "Runs Running ZZQ";
    let slug = slugify(title);
    let store = new_store();
    store
        .lock()
        .unwrap()
        .insert("id".into(), make_routine("id", title));

    let workbenches = crate::paths::workbenches_dir();
    std::fs::create_dir_all(workbenches.join(format!("{slug}-1000"))).unwrap();

    with_tmux_alive(|| {
        let runs = svc_list_runs(&store, "id").unwrap();
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].status, RunStatus::Running);
        assert_eq!(runs[0].exit_code, None);
    });
}

#[test]
fn svc_run_log_missing_routine_not_found() {
    let _home = TempHome::set();
    assert!(matches!(
        svc_run_log(&new_store(), "nope", "whatever-1"),
        Err(AppError::NotFound)
    ));
}

#[test]
fn svc_run_log_not_found_for_unparseable_workbench_name() {
    let _home = TempHome::set();
    let title = "Run Log Bad Name ZZQ";
    let store = new_store();
    store
        .lock()
        .unwrap()
        .insert("id".into(), make_routine("id", title));

    assert!(matches!(
        svc_run_log(&store, "id", "not-a-workbench"),
        Err(AppError::NotFound)
    ));
}

#[test]
fn svc_run_log_not_found_for_foreign_workbench() {
    let _home = TempHome::set();
    let title = "Run Log Foreign ZZQ";
    let store = new_store();
    store
        .lock()
        .unwrap()
        .insert("id".into(), make_routine("id", title));

    assert!(matches!(
        svc_run_log(&store, "id", "some-other-routine-9999"),
        Err(AppError::NotFound)
    ));
}

#[test]
fn svc_run_log_empty_when_agent_log_missing() {
    let _home = TempHome::set();
    let title = "Run Log Missing File ZZQ";
    let slug = slugify(title);
    let store = new_store();
    store
        .lock()
        .unwrap()
        .insert("id".into(), make_routine("id", title));

    let workbench = format!("{slug}-1000");
    std::fs::create_dir_all(crate::paths::workbenches_dir().join(&workbench)).unwrap();

    assert_eq!(svc_run_log(&store, "id", &workbench).unwrap(), "");
}

#[test]
fn svc_run_log_returns_specific_workbench_log() {
    let _home = TempHome::set();
    let title = "Run Log Exact ZZQ";
    let slug = slugify(title);
    let store = new_store();
    store
        .lock()
        .unwrap()
        .insert("id".into(), make_routine("id", title));

    let workbenches = crate::paths::workbenches_dir();
    let older = format!("{slug}-1000");
    let newer = format!("{slug}-2000");
    std::fs::create_dir_all(workbenches.join(&older)).unwrap();
    std::fs::create_dir_all(workbenches.join(&newer)).unwrap();
    std::fs::write(workbenches.join(&older).join("agent.log"), "older run").unwrap();
    std::fs::write(workbenches.join(&newer).join("agent.log"), "newer run").unwrap();

    // Explicitly asking for the *older* run's log must not fall back to the newest, unlike
    // `svc_logs`.
    assert_eq!(svc_run_log(&store, "id", &older).unwrap(), "older run");
    assert_eq!(svc_run_log(&store, "id", &newer).unwrap(), "newer run");
}

#[test]
fn svc_list_all_runs_merges_across_routines_newest_first() {
    let _home = TempHome::set();
    let title_a = "Fleet Runs A ZZQ";
    let title_b = "Fleet Runs B ZZQ";
    let slug_a = slugify(title_a);
    let slug_b = slugify(title_b);
    let store = new_store();
    store
        .lock()
        .unwrap()
        .insert("a-id".into(), make_routine("a-id", title_a));
    store
        .lock()
        .unwrap()
        .insert("b-id".into(), make_routine("b-id", title_b));

    let workbenches = crate::paths::workbenches_dir();
    std::fs::create_dir_all(workbenches.join(format!("{slug_a}-1000"))).unwrap();
    std::fs::create_dir_all(workbenches.join(format!("{slug_b}-3000"))).unwrap();
    std::fs::create_dir_all(workbenches.join(format!("{slug_a}-2000"))).unwrap();
    // A workbench with no matching routine (deleted since) must not appear.
    std::fs::create_dir_all(workbenches.join("some-deleted-routine-9999")).unwrap();
    // Not a `{slug}-{ts}` directory at all: parse_workbench_name returns None.
    std::fs::create_dir_all(workbenches.join("not-a-workbench-name")).unwrap();

    let runs = svc_list_all_runs(&store, None);
    assert_eq!(
        runs.iter().map(|run| run.started_at).collect::<Vec<_>>(),
        vec![3000, 2000, 1000]
    );
    assert_eq!(runs[0].routine_id, "b-id");
    assert_eq!(runs[0].routine_title, title_b);
    assert_eq!(runs[1].routine_id, "a-id");
    assert_eq!(runs[2].routine_id, "a-id");
}

#[test]
fn svc_list_all_runs_truncates_to_limit() {
    let _home = TempHome::set();
    let title = "Fleet Runs Limit ZZQ";
    let slug = slugify(title);
    let store = new_store();
    store
        .lock()
        .unwrap()
        .insert("id".into(), make_routine("id", title));

    let workbenches = crate::paths::workbenches_dir();
    for ts in [1000, 2000, 3000] {
        std::fs::create_dir_all(workbenches.join(format!("{slug}-{ts}"))).unwrap();
    }

    let runs = svc_list_all_runs(&store, Some(2));
    assert_eq!(runs.len(), 2);
    assert_eq!(runs[0].started_at, 3000);
    assert_eq!(runs[1].started_at, 2000);
}

#[test]
fn svc_list_all_runs_empty_when_workbenches_dir_absent() {
    let _home = TempHome::set();
    assert_eq!(svc_list_all_runs(&new_store(), None), vec![]);
}

#[test]
fn svc_list_runs_merges_persisted_history_with_live_workbenches() {
    use crate::routines::run_history::{append_persisted_run, PersistedRun};

    let _home = TempHome::set();
    let title = "Runs Persisted ZZQ";
    let store = new_store();
    store
        .lock()
        .unwrap()
        .insert("id".into(), make_routine("id", title));

    let slug = slugify(title);
    std::fs::create_dir_all(crate::paths::workbenches_dir().join(format!("{slug}-3000"))).unwrap();
    append_persisted_run(
        "id",
        &PersistedRun {
            workbench: format!("{slug}-1000"),
            started_at: 1000,
            finished_at: 1005,
            status: RunStatus::Success,
            exit_code: Some(0),
        },
    );

    let runs = svc_list_runs(&store, "id").unwrap();
    assert_eq!(
        runs.iter().map(|run| run.started_at).collect::<Vec<_>>(),
        vec![3000, 1000],
        "the live workbench and the persisted (already-reaped) run both appear, newest first"
    );
    assert_eq!(runs[1].status, RunStatus::Success);
    assert_eq!(runs[1].finished_at, Some(1005));
}

#[test]
fn svc_list_all_runs_merges_persisted_history_across_routines() {
    use crate::routines::run_history::{append_persisted_run, PersistedRun};

    let _home = TempHome::set();
    let title = "Fleet Runs Persisted ZZQ";
    let store = new_store();
    store
        .lock()
        .unwrap()
        .insert("id".into(), make_routine("id", title));

    append_persisted_run(
        "id",
        &PersistedRun {
            workbench: format!("{}-1000", slugify(title)),
            started_at: 1000,
            finished_at: 1005,
            status: RunStatus::Failed,
            exit_code: Some(1),
        },
    );
    // A persisted run whose routine has since been deleted must not appear.
    append_persisted_run(
        "deleted-routine-id",
        &PersistedRun {
            workbench: "some-slug-2000".into(),
            started_at: 2000,
            finished_at: 2005,
            status: RunStatus::Success,
            exit_code: Some(0),
        },
    );

    let runs = svc_list_all_runs(&store, None);
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].routine_id, "id");
    assert_eq!(runs[0].status, RunStatus::Failed);
    assert_eq!(runs[0].exit_code, Some(1));
}
