#![allow(
    clippy::wildcard_imports,
    clippy::too_many_arguments,
    clippy::needless_pass_by_value,
    dead_code,
    reason = "split-out module preserves existing code while keeping files under the linecheck limit"
)]
use super::*;

#[test]
fn svc_create_rejects_malformed_agent_config() {
    let _home = TempHome::set();
    // A referenced agent whose `<name>.toml` is present but unparseable is rejected at create time
    // with `BadRequest` quoting the parse error — not silently skipped at fire time.
    let agent_name = "svc-create-malformed-agent-zzz";
    std::fs::create_dir_all(crate::paths::agents_dir()).unwrap();
    let cfg = crate::paths::agent_toml_path(agent_name);
    std::fs::write(&cfg, "command = [\n").unwrap();

    let store = new_store();
    let result = svc_create(
        &store,
        CreateRoutineRequest {
            model: None,
            schedule: "@daily".into(),
            schedules: vec![],
            title: "Svc Create Malformed ZZZ".into(),
            agent: agent_name.into(),
            prompt: "p".into(),
            goal: None,
            repositories: vec![],
            machines: vec![crate::machine::current_machine()],
            enabled: true,
            ttl_secs: None,
            max_runtime_secs: None,
            tags: vec![],
            env: std::collections::HashMap::new(),
            failure_threshold: None,
        },
    );
    match result {
        Err(AppError::BadRequest(msg)) => assert!(msg.contains("malformed config")),
        other => panic!("expected BadRequest, got {other:?}"),
    }
}

#[test]
fn svc_create_rejects_unreadable_agent_config() {
    // A referenced agent whose `<name>.toml` is present but unreadable (here a directory at the
    // path, which reads back as a non-`NotFound` I/O error) is rejected at create time with
    // `BadRequest` — not accepted and left as a green-dot routine that never fires.
    let agent_name = "svc-create-unreadable-agent-zzz";
    std::fs::create_dir_all(crate::paths::agents_dir()).unwrap();
    let cfg = crate::paths::agent_toml_path(agent_name);
    std::fs::create_dir_all(&cfg).unwrap();

    let store = new_store();
    let result = svc_create(
        &store,
        CreateRoutineRequest {
            model: None,
            schedule: "@daily".into(),
            schedules: vec![],
            title: "Svc Create Unreadable ZZZ".into(),
            agent: agent_name.into(),
            prompt: "p".into(),
            goal: None,
            repositories: vec![],
            machines: vec![],
            tags: vec![],
            enabled: true,
            ttl_secs: None,
            max_runtime_secs: None,
            env: std::collections::HashMap::new(),
            failure_threshold: None,
        },
    );
    match result {
        Err(AppError::BadRequest(msg)) => assert!(msg.contains("unreadable config")),
        other => panic!("expected BadRequest, got {other:?}"),
    }

    std::fs::remove_dir_all(&cfg).unwrap();
}

#[test]
fn svc_update_rejects_malformed_agent_config() {
    let _home = TempHome::set();
    // The same rejection applies when an update switches a routine to a malformed agent.
    let agent_name = "svc-update-malformed-agent-zzz";
    std::fs::create_dir_all(crate::paths::agents_dir()).unwrap();
    let cfg = crate::paths::agent_toml_path(agent_name);
    std::fs::write(&cfg, "command = [\n").unwrap();

    let title = "Svc Update Malformed ZZZ";
    let store = new_store();
    let routine = make_routine("upd-mal-id", title, 1, 1);
    crate::routine_storage::write_routine(&routine).unwrap();
    store.lock().unwrap().insert("upd-mal-id".into(), routine);

    let result = svc_update(
        &store,
        "upd-mal-id",
        UpdateRoutineRequest {
            model: None,
            schedule: None,
            schedules: None,
            title: None,
            agent: Some(agent_name.into()),
            prompt: None,
            goal: None,
            repositories: None,
            machines: None,
            enabled: None,
            ttl_secs: None,
            max_runtime_secs: None,
            tags: None,
            env: None,
            failure_threshold: None,
        },
    );
    match result {
        Err(AppError::BadRequest(msg)) => assert!(msg.contains("malformed config")),
        other => panic!("expected BadRequest, got {other:?}"),
    }
}
include!("service_slug_tests_part4.rs");
