#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use super::*;

use crate::routines::new_store;

struct TempHome(std::path::PathBuf);

impl TempHome {
    fn set() -> Self {
        let dir = std::env::temp_dir().join(format!("moadim-svctest-{}", uuid::Uuid::new_v4()));
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

fn make_routine(id: &str, title: &str, created_at: u64, updated_at: u64) -> Routine {
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
        created_at,
        updated_at,
        last_manual_trigger_at: None,
        last_scheduled_trigger_at: None,
        snoozed_until: None,
        skip_runs: None,
        power_saving: false,
        tags: vec![],
        ttl_secs: None,
        max_runtime_secs: None,
    }
}

#[test]
fn svc_logs_returns_newest_workbench_log() {
    let _home = TempHome::set();
    // Covers the newest-workbench selection inside `svc_logs`: with two valid
    // `{slug}-{ts}` workbench directories, the higher timestamp wins and its
    // `agent.log` contents are returned.
    let title = "Svc Logs Newest ZZZ";
    let slug = slugify(title);
    let store = new_store();
    let mut routine = make_routine("logs-id", title, 1, 1);
    routine.repositories = vec![Repository {
        repository: "https://example.com/r.git".into(),
        branch: None,
    }];
    store.lock().unwrap().insert("logs-id".into(), routine);

    let workbenches = crate::paths::workbenches_dir();
    std::fs::create_dir_all(&workbenches).unwrap();
    let older = workbenches.join(format!("{slug}-1000"));
    let newer = workbenches.join(format!("{slug}-2000"));
    std::fs::create_dir_all(&older).unwrap();
    std::fs::create_dir_all(&newer).unwrap();
    std::fs::write(older.join("agent.log"), "old log contents").unwrap();
    std::fs::write(newer.join("agent.log"), "new log contents").unwrap();

    let logs = svc_logs(&store, "logs-id").unwrap();
    assert_eq!(logs, "new log contents");
}

#[test]
fn svc_logs_skips_foreign_and_unparseable_workbenches() {
    let _home = TempHome::set();
    // Exercises the read_dir loop body across every arm: a workbench whose name
    // does not parse as `{slug}-{ts}` (parser returns None → skipped), a workbench
    // that parses but belongs to a different routine (`dir_slug != slug` → skipped),
    // and finally this routine's own workbench whose log is returned.
    let title = "Svc Logs Mixed ZZQ";
    let slug = slugify(title);
    let store = new_store();
    let routine = make_routine("logs-mixed-id", title, 1, 1);
    store
        .lock()
        .unwrap()
        .insert("logs-mixed-id".into(), routine);

    let workbenches = crate::paths::workbenches_dir();
    std::fs::create_dir_all(&workbenches).unwrap();

    // Not a `{slug}-{ts}` directory at all: parse_workbench_name returns None.
    let unparseable = workbenches.join("not-a-workbench-name");
    std::fs::create_dir_all(&unparseable).unwrap();
    std::fs::write(unparseable.join("agent.log"), "ignored").unwrap();

    // A well-formed workbench owned by a *different* routine slug.
    let foreign = workbenches.join("some-other-routine-9999");
    std::fs::create_dir_all(&foreign).unwrap();
    std::fs::write(foreign.join("agent.log"), "foreign log").unwrap();

    // This routine's own workbench.
    let mine = workbenches.join(format!("{slug}-4242"));
    std::fs::create_dir_all(&mine).unwrap();
    std::fs::write(mine.join("agent.log"), "mine log contents").unwrap();

    let logs = svc_logs(&store, "logs-mixed-id").unwrap();
    assert_eq!(logs, "mine log contents");
}

#[test]
fn svc_logs_empty_when_workbenches_dir_absent() {
    let _home = TempHome::set();
    // Covers the `read_dir` error path in `svc_logs`: the fresh temp home has no `workbenches`
    // subdirectory, so `std::fs::read_dir` returns Err and the loop is skipped entirely. With no
    // workbench found, the function returns an empty string.
    let title = "Svc Logs No Workbenches ZZQ";
    let store = new_store();
    store.lock().unwrap().insert(
        "logs-empty-id".into(),
        make_routine("logs-empty-id", title, 1, 1),
    );

    assert!(!crate::paths::workbenches_dir().exists());

    let logs = svc_logs(&store, "logs-empty-id").unwrap();
    assert_eq!(logs, "");
}

#[test]
fn svc_logs_missing_routine_not_found() {
    let _home = TempHome::set();
    assert!(matches!(
        svc_logs(&new_store(), "nope"),
        Err(AppError::NotFound)
    ));
}
