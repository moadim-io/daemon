#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use super::*;

fn make_routine(id: &str) -> Routine {
    Routine {
        model: None,
        id: id.to_string(),
        schedule: "@daily".to_string(),
        title: "My Routine".to_string(),
        agent: "claude".to_string(),
        prompt: "do the thing".to_string(),
        goal: None,
        repositories: vec![Repository {
            repository: "https://github.com/octocat/Hello-World".to_string(),
            branch: Some("master".to_string()),
        }],
        machines: vec![crate::machine::current_machine()],
        enabled: true,
        source: "managed".to_string(),
        auto_pull: true,
        created_at: 0,
        updated_at: 0,
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
fn available_agents_lists_sorted_toml_stems() {
    let dir = std::env::temp_dir().join("moadim-agents-list-test");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("zeta.toml"), "command = \"z\"\nargs = []\n").unwrap();
    std::fs::write(dir.join("alpha.toml"), "command = \"a\"\nargs = []\n").unwrap();
    // non-toml files are ignored
    std::fs::write(dir.join("notes.txt"), "ignore me").unwrap();

    assert_eq!(
        available_agents_in(&dir),
        vec!["alpha".to_string(), "zeta".to_string()]
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn available_agents_falls_back_to_builtins_when_missing() {
    let dir = std::env::temp_dir().join("moadim-agents-missing-test");
    let _ = std::fs::remove_dir_all(&dir);
    // directory does not exist → built-in defaults (declaration order)
    assert_eq!(
        available_agents_in(&dir),
        vec![
            "claude".to_string(),
            "codex".to_string(),
            "hermes".to_string()
        ]
    );
}

#[test]
fn routine_response_schedule_description() {
    let resp = RoutineResponse::from_routine(make_routine("x"));
    assert!(resp.schedule_description.is_some());
    // file_path is based on the slugified title ("My Routine" → "my-routine")
    assert!(resp.file_path.contains("my-routine"));
}

#[test]
fn routine_response_schedule_description_none_for_reboot() {
    let mut routine = make_routine("x");
    routine.schedule = "@reboot".to_string();
    let resp = RoutineResponse::from_routine(routine);
    assert!(resp.schedule_description.is_none());
}

#[test]
fn routine_response_schedule_description_includes_timezone() {
    let resp = RoutineResponse::from_routine(make_routine("x"));
    // When the local timezone resolves, the description is suffixed with it
    // (e.g. "... (Asia/Jerusalem)") and the dedicated field is populated.
    if let Some(tz) = &resp.timezone {
        let desc = resp
            .schedule_description
            .as_ref()
            .expect("parseable schedule should have a description");
        assert!(
            desc.ends_with(&format!("({tz})")),
            "schedule_description {desc:?} should end with the timezone ({tz})"
        );
    }
}

#[test]
fn svc_create_invalid_cron_rejected() {
    let store = new_store();
    let req = CreateRoutineRequest {
        auto_pull: true,
        schedule: "not-a-cron".into(),
        title: "t".into(),
        agent: "claude".into(),
        model: None,
        prompt: "p".into(),
        goal: None,
        repositories: vec![],
        machines: vec![crate::machine::current_machine()],
        enabled: true,
        ttl_secs: None,
        max_runtime_secs: None,
        tags: vec![],
    };
    assert!(svc_create(&store, req).is_err());
}

#[test]
fn svc_create_update_delete_lifecycle() {
    let store = new_store();
    let created = svc_create(
        &store,
        CreateRoutineRequest {
            auto_pull: true,
            model: None,
            schedule: "@daily".into(),
            title: "Cov Routine".into(),
            agent: "claude".into(),
            prompt: "p".into(),
            goal: None,
            repositories: vec![],
            machines: vec![crate::machine::current_machine()],
            enabled: true,
            ttl_secs: None,
            max_runtime_secs: None,
            tags: vec![],
        },
    )
    .unwrap();
    let id = created.routine.id.clone();
    // folder is slug of the title, not the UUID
    assert!(crate::paths::routine_toml_path("cov-routine").exists());
    assert!(crate::paths::routine_compiled_prompt_path("cov-routine").exists());

    let updated = svc_update(
        &store,
        &id,
        UpdateRoutineRequest {
            auto_pull: None,
            model: None,
            schedule: Some("@weekly".into()),
            title: Some("Renamed".into()),
            agent: Some("codex".into()),
            prompt: Some("p2".into()),
            goal: None,
            repositories: Some(vec![Repository {
                repository: "r".into(),
                branch: None,
            }]),
            machines: None,
            enabled: Some(false),
            ttl_secs: None,
            max_runtime_secs: None,
            tags: None,
        },
    )
    .unwrap();
    assert_eq!(updated.routine.schedule, "@weekly");
    assert_eq!(updated.routine.title, "Renamed");
    assert_eq!(updated.routine.agent, "codex");
    assert!(!updated.routine.enabled);

    svc_delete(&store, &id).unwrap();
    // after rename to "Renamed" and delete, the slug dir is gone
    assert!(!crate::paths::routine_dir("renamed").exists());
}

#[test]
fn svc_update_not_found() {
    let req = UpdateRoutineRequest {
        auto_pull: None,
        schedule: None,
        title: Some("x".into()),
        agent: None,
        model: None,
        prompt: None,
        goal: None,
        repositories: None,
        machines: None,
        enabled: None,
        ttl_secs: None,
        max_runtime_secs: None,
        tags: None,
    };
    assert!(svc_update(&new_store(), "missing", req).is_err());
}

#[test]
fn svc_update_invalid_cron_rejected() {
    let store = new_store();
    store
        .lock()
        .unwrap()
        .insert("id".into(), make_routine("id"));
    let req = UpdateRoutineRequest {
        auto_pull: None,
        schedule: Some("bad".into()),
        title: None,
        agent: None,
        model: None,
        prompt: None,
        goal: None,
        repositories: None,
        machines: None,
        enabled: None,
        ttl_secs: None,
        max_runtime_secs: None,
        tags: None,
    };
    assert!(svc_update(&store, "id", req).is_err());
}

#[test]
fn svc_delete_not_found() {
    assert!(svc_delete(&new_store(), "missing").is_err());
}

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

    // `spawn_routine_command` runs auto-pull synchronously before the fire-and-forget shell
    // spawn, so by the time `svc_trigger` returns it has already attempted (and, with no
    // `MOADIM_GIT_BIN` shim configured, failed to) sync this routine's repository — raising a
    // visible flag (#1132) rather than failing silently.
    let flags = crate::routines::flags::list_flags("trigger-cov-title-zzz");
    assert!(
        flags
            .iter()
            .any(|flag| flag.flag_type == "auto_pull_failed"),
        "expected an auto_pull_failed flag, got {flags:?}"
    );

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
fn svc_trigger_skips_auto_pull_when_disabled() {
    // Agent config with a harmless command so the spawned shell exits immediately.
    let agent_name = "trigger-cov-agent-no-pull-zzz";
    std::fs::create_dir_all(crate::paths::agents_dir()).unwrap();
    let cfg = crate::paths::agent_toml_path(agent_name);
    std::fs::write(&cfg, "command = \"true\"\nargs = []\n").unwrap();

    let store = new_store();
    let title = "Trigger Cov No Pull Title ZZZ";
    let mut routine = make_routine("trig-no-pull-cfg");
    routine.title = title.into();
    routine.agent = agent_name.into();
    routine.auto_pull = false;
    store
        .lock()
        .unwrap()
        .insert("trig-no-pull-cfg".into(), routine.clone());
    crate::routine_storage::write_routine(&routine).unwrap();

    svc_trigger(&store, "trig-no-pull-cfg").unwrap();

    // `auto_pull: false` must skip the sync entirely — no flag, even with a repository
    // configured and no `MOADIM_GIT_BIN` shim (which would otherwise make the sync fail).
    let slug = slugify(title);
    let flags = crate::routines::flags::list_flags(&slug);
    assert!(flags.is_empty(), "expected no flags, got {flags:?}");

    // Let the fire-and-forget shell create its workbench, then clean everything up.
    std::thread::sleep(std::time::Duration::from_millis(150));
    std::fs::remove_file(&cfg).unwrap();
    crate::routine_storage::remove_routine_dir(&slug).unwrap();
    let prefix = format!("{slug}-");
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
