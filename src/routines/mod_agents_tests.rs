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
        schedules: vec![],
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
        env: std::collections::HashMap::new(),
        auto_disabled_reason: None,
        consecutive_failures: 0,
        failure_threshold: None,
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
            "hermes".to_string(),
            "pi".to_string()
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
        schedule: "not-a-cron".into(),
        schedules: vec![],
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
        env: std::collections::HashMap::new(),
        failure_threshold: None,
    };
    assert!(svc_create(&store, req).is_err());
}

#[test]
fn svc_create_update_delete_lifecycle() {
    let store = new_store();
    let created = svc_create(
        &store,
        CreateRoutineRequest {
            model: None,
            schedule: "@daily".into(),
            schedules: vec![],
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
            env: std::collections::HashMap::new(),
            failure_threshold: None,
        },
    )
    .unwrap();
    let id = created.routine.id;
    // folder is slug of the title, not the UUID
    assert!(crate::paths::routine_toml_path("cov-routine").exists());
    assert!(crate::paths::routine_compiled_prompt_path("cov-routine").exists());

    let updated = svc_update(
        &store,
        &id,
        UpdateRoutineRequest {
            model: None,
            schedule: Some("@weekly".into()),
            schedules: None,
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
            env: None,
            failure_threshold: None,
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
        schedule: None,
        schedules: None,
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
        env: None,
        failure_threshold: None,
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
        schedule: Some("bad".into()),
        schedules: None,
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
        env: None,
        failure_threshold: None,
    };
    assert!(svc_update(&store, "id", req).is_err());
}

#[test]
fn svc_delete_not_found() {
    assert!(svc_delete(&new_store(), "missing").is_err());
}

#[allow(
    clippy::missing_docs_in_private_items,
    reason = "split-out module keeps the file under the linecheck limit"
)]
#[path = "mod_agents_tests_part2.rs"]
mod mod_agents_tests_part2;
