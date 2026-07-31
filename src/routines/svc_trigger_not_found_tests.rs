#![allow(
    clippy::wildcard_imports,
    clippy::too_many_arguments,
    clippy::needless_pass_by_value,
    dead_code,
    reason = "split-out module preserves existing code while keeping files under the linecheck limit"
)]
use super::*;

#[test]
fn svc_trigger_not_found() {
    assert!(svc_trigger(&new_store(), "missing").is_err());
}

#[test]
fn svc_trigger_records_time_without_agent_config() {
    // Agent name that has no config file → records trigger, does not spawn.
    let store = new_store();
    let mut routine = make_routine("trig-id");
    routine.agent = "no-such-agent-xyz".into();
    store.lock().unwrap().insert("trig-id".into(), routine);
    let triggered = svc_trigger(&store, "trig-id").unwrap();
    assert!(triggered.last_manual_trigger_at.is_some());
    // folder is slug of "My Routine"
    crate::routine_storage::remove_routine_dir("my-routine").unwrap();
}

#[test]
fn load_agent_command_missing_returns_missing_error() {
    assert!(matches!(
        load_agent_command("definitely-not-an-agent-zzz"),
        Err(crate::routines::AgentLoadError::Missing)
    ));
}

#[test]
fn svc_trigger_with_agent_config_spawns() {
    // Agent config with a harmless command so the spawned shell exits immediately.
    let agent_name = "trigger-cov-agent-zzz";
    std::fs::create_dir_all(crate::paths::agents_dir()).unwrap();
    let cfg = crate::paths::agent_toml_path(agent_name);
    std::fs::write(&cfg, "command = \"true\"\nargs = []\n").unwrap();

    let store = new_store();
    let title = "Trigger Cov Title ZZZ";
    let mut routine = make_routine("trig-cfg");
    routine.title = title.into();
    routine.agent = agent_name.into();
    store
        .lock()
        .unwrap()
        .insert("trig-cfg".into(), routine.clone());
    crate::routine_storage::write_routine(&routine).unwrap();

    let triggered = svc_trigger(&store, "trig-cfg").unwrap();
    assert!(triggered.last_manual_trigger_at.is_some());

    // Let the fire-and-forget shell create its workbench, then clean everything up.
    std::thread::sleep(std::time::Duration::from_millis(150));
    std::fs::remove_file(&cfg).unwrap();
    // folder is slug of title "Trigger Cov Title ZZZ"
    crate::routine_storage::remove_routine_dir("trigger-cov-title-zzz").unwrap();
    let prefix = format!("{}-", slugify(title));
    if let Ok(entries) = std::fs::read_dir(crate::paths::workbenches_dir()) {
        for entry in entries.flatten() {
            if entry.file_name().to_string_lossy().starts_with(&prefix) {
                let _ = std::fs::remove_dir_all(entry.path());
            }
        }
    }
}

#[test]
fn create_request_defaults_enabled_true() {
    let json = r#"{"schedule":"@daily","title":"t","agent":"a","prompt":"p"}"#;
    let req: CreateRoutineRequest = serde_json::from_str(json).unwrap();
    assert!(req.enabled);
    assert!(req.repositories.is_empty());
    assert!(bool_true());
}

#[test]
fn svc_logs_not_found() {
    assert!(svc_logs(&new_store(), "missing").is_err());
}

#[test]
fn svc_logs_empty_when_no_workbench() {
    let store = new_store();
    let mut routine = make_routine("logs-id");
    routine.title = "Unlikely Title For Logs 9988".into();
    store.lock().unwrap().insert("logs-id".into(), routine);
    assert_eq!(svc_logs(&store, "logs-id").unwrap().content, "");
}

#[test]
fn svc_logs_returns_newest_workbench_log() {
    let store = new_store();
    let mut routine = make_routine("logs-newest");
    routine.title = "Logs Cov Newest AAA".into();
    let slug = slugify(&routine.title);
    store.lock().unwrap().insert("logs-newest".into(), routine);

    let wb = crate::paths::workbenches_dir();
    let old = wb.join(format!("{slug}-1000"));
    let new = wb.join(format!("{slug}-2000"));
    std::fs::create_dir_all(&old).unwrap();
    std::fs::create_dir_all(&new).unwrap();
    std::fs::write(old.join("agent.log"), "old-log").unwrap();
    std::fs::write(new.join("agent.log"), "new-log").unwrap();

    assert_eq!(svc_logs(&store, "logs-newest").unwrap().content, "new-log");

    std::fs::remove_dir_all(&old).unwrap();
    std::fs::remove_dir_all(&new).unwrap();
}

#[test]
fn svc_logs_empty_when_newest_has_no_log_file() {
    let store = new_store();
    let mut routine = make_routine("logs-nofile");
    routine.title = "Logs Cov NoFile BBB".into();
    let slug = slugify(&routine.title);
    store.lock().unwrap().insert("logs-nofile".into(), routine);

    let dir = crate::paths::workbenches_dir().join(format!("{slug}-3000"));
    std::fs::create_dir_all(&dir).unwrap();
    assert_eq!(svc_logs(&store, "logs-nofile").unwrap().content, "");
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn svc_logs_ignores_other_routine_with_shared_slug_prefix() {
    let store = new_store();
    let mut routine = make_routine("logs-prefix");
    routine.title = "Logs Cov Prefix ZZQ".into();
    let slug = slugify(&routine.title); // "logs-cov-prefix-zzq"
    store.lock().unwrap().insert("logs-prefix".into(), routine);

    let wb = crate::paths::workbenches_dir();
    let mine = wb.join(format!("{slug}-1000"));
    // Belongs to a *different* routine whose slug is `{slug}-extra`. Its name shares
    // the bare `{slug}-` prefix and sorts lexicographically *after* `mine`, so the old
    // prefix match would wrongly return its log.
    let other = wb.join(format!("{slug}-extra-2000"));
    std::fs::create_dir_all(&mine).unwrap();
    std::fs::create_dir_all(&other).unwrap();
    std::fs::write(mine.join("agent.log"), "mine").unwrap();
    std::fs::write(other.join("agent.log"), "not-mine").unwrap();

    assert_eq!(svc_logs(&store, "logs-prefix").unwrap().content, "mine");

    std::fs::remove_dir_all(&mine).unwrap();
    std::fs::remove_dir_all(&other).unwrap();
}
