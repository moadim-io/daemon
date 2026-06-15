#![allow(clippy::missing_docs_in_private_items)]

use super::*;

fn make_routine(id: &str) -> Routine {
    Routine {
        id: id.to_string(),
        schedule: "@daily".to_string(),
        title: "My Routine".to_string(),
        agent: "claude".to_string(),
        prompt: "do the thing".to_string(),
        repositories: vec![Repository {
            repository: "https://github.com/octocat/Hello-World".to_string(),
            branch: Some("master".to_string()),
        }],
        enabled: true,
        source: "managed".to_string(),
        created_at: 0,
        updated_at: 0,
        last_triggered_at: None,
    }
}

#[test]
fn slugify_basic() {
    assert_eq!(slugify("My Routine"), "my-routine");
    assert_eq!(slugify("  Hello,  World! "), "hello-world");
    assert_eq!(slugify("UPPER_case-123"), "upper-case-123");
}

#[test]
fn slugify_empty_falls_back() {
    assert_eq!(slugify(""), "routine");
    assert_eq!(slugify("---"), "routine");
    assert_eq!(slugify("!@#$"), "routine");
}

#[test]
fn compose_prompt_lists_repos_and_prompt() {
    let r = make_routine("x");
    let p = compose_prompt(&r);
    assert!(p.contains("# Workbench"));
    assert!(p.contains("https://github.com/octocat/Hello-World (branch master)"));
    assert!(p.contains("do the thing"));
}

#[test]
fn compose_prompt_repo_without_branch() {
    let mut r = make_routine("x");
    r.repositories = vec![Repository {
        repository: "git@example.com:a/b".to_string(),
        branch: None,
    }];
    let p = compose_prompt(&r);
    assert!(p.contains("- git@example.com:a/b\n"));
}

#[test]
fn substitute_replaces_placeholders() {
    assert_eq!(
        substitute("read {prompt_file} in {workbench}", ".", "prompt.md"),
        "read prompt.md in ."
    );
    assert_eq!(
        substitute("claude {prompt}", ".", "prompt.md"),
        r#"claude "$(cat prompt.md)""#
    );
}

#[test]
fn shell_quote_wraps_and_escapes() {
    assert_eq!(shell_quote("abc"), "'abc'");
    assert_eq!(shell_quote("a'b"), "'a'\\''b'");
}

#[test]
fn build_routine_command_contains_expected_pieces() {
    let r = make_routine("rid");
    let agent = AgentCommand {
        command: "claude".to_string(),
        args: vec![
            "--dangerously-skip-permissions".to_string(),
            "{prompt}".to_string(),
        ],
        setup: None,
    };
    let cmd = build_routine_command(&r, &agent);
    assert!(cmd.contains("tmux new-session -d -s \"$SESS\" -c \"$WB\""));
    // bakes a PATH export so cron's minimal PATH does not hide tmux/claude
    assert!(cmd.contains("export PATH="));
    // stay well under cron's per-line length limit (~1000 chars) — a full inherited PATH blew past it
    assert!(
        cmd.len() < 1000,
        "crontab line too long: {} chars",
        cmd.len()
    );
    // prompt passed as a process argument via command substitution, no send-keys
    assert!(cmd.contains(r#""$(cat prompt.md)""#));
    assert!(!cmd.contains("send-keys"));
    assert!(!cmd.contains("capture-pane"));
    assert!(cmd.contains("tmux pipe-pane"));
    assert!(cmd.contains("SLUG='my-routine'"));
    // single line — no newlines
    assert!(!cmd.contains('\n'));
}

#[test]
fn build_routine_command_substitutes_arg_placeholders() {
    let r = make_routine("rid");
    let agent = AgentCommand {
        command: "codex".to_string(),
        args: vec!["exec".to_string(), "{prompt_file}".to_string()],
        setup: None,
    };
    let cmd = build_routine_command(&r, &agent);
    assert!(cmd.contains("'codex exec prompt.md'"));
}

#[test]
fn build_routine_command_inserts_setup_before_launch() {
    let r = make_routine("rid");
    let agent = AgentCommand {
        command: "claude".to_string(),
        args: vec!["{prompt}".to_string()],
        setup: Some("seed-trust \"$WB\"".to_string()),
    };
    let cmd = build_routine_command(&r, &agent);
    let setup_at = cmd.find("seed-trust").expect("setup present");
    let launch_at = cmd.find("tmux new-session").expect("launch present");
    // setup runs before the agent launches
    assert!(setup_at < launch_at);
    // inserted verbatim (not shell-quoted), $WB left for the runtime shell to expand
    assert!(cmd.contains("seed-trust \"$WB\""));
}

#[test]
fn ensure_default_agents_writes_parsable_configs() {
    let dir = std::env::temp_dir().join("moadim-agents-defaults-test");
    let _ = std::fs::remove_dir_all(&dir);
    ensure_default_agents_in(&dir);

    // claude default parses and carries the unattended-launch setup seed
    let claude_text = std::fs::read_to_string(dir.join("claude.toml")).unwrap();
    let claude: AgentCommand = toml::from_str(&claude_text).unwrap();
    assert_eq!(claude.command, "claude");
    assert!(claude.args.contains(&"{prompt}".to_string()));
    let setup = claude.setup.expect("claude default has a setup step");
    assert!(setup.contains("hasTrustDialogAccepted"));
    assert!(setup.contains("disabledMcpjsonServers"));

    // codex default parses and passes the prompt file as an argument
    let codex: AgentCommand =
        toml::from_str(&std::fs::read_to_string(dir.join("codex.toml")).unwrap()).unwrap();
    assert_eq!(codex.command, "codex");
    assert!(codex.args.contains(&"{prompt_file}".to_string()));

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn ensure_default_agents_does_not_overwrite_existing() {
    let dir = std::env::temp_dir().join("moadim-agents-preserve-test");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("claude.toml"), "command = \"mine\"\nargs = []\n").unwrap();

    ensure_default_agents_in(&dir);

    // user file untouched, codex default still seeded
    assert_eq!(
        std::fs::read_to_string(dir.join("claude.toml")).unwrap(),
        "command = \"mine\"\nargs = []\n"
    );
    assert!(dir.join("codex.toml").exists());

    let _ = std::fs::remove_dir_all(&dir);
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
    // directory does not exist → built-in defaults
    assert_eq!(
        available_agents_in(&dir),
        vec!["claude".to_string(), "codex".to_string()]
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
    let mut r = make_routine("x");
    r.schedule = "@reboot".to_string();
    let resp = RoutineResponse::from_routine(r);
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
fn svc_get_not_found() {
    assert!(svc_get(&new_store(), "missing").is_err());
}

#[test]
fn svc_list_empty() {
    assert!(svc_list(&new_store()).is_empty());
}

#[test]
fn svc_list_sorted_by_created_at() {
    let store = new_store();
    let mut early = make_routine("early");
    early.created_at = 10;
    let mut late = make_routine("late");
    late.created_at = 20;
    store.lock().unwrap().insert("late".into(), late);
    store.lock().unwrap().insert("early".into(), early);
    let list = svc_list(&store);
    assert_eq!(list[0].routine.id, "early");
    assert_eq!(list[1].routine.id, "late");
}

#[test]
fn svc_create_invalid_cron_rejected() {
    let store = new_store();
    let req = CreateRoutineRequest {
        schedule: "not-a-cron".into(),
        title: "t".into(),
        agent: "claude".into(),
        prompt: "p".into(),
        repositories: vec![],
        enabled: true,
    };
    assert!(svc_create(&store, req).is_err());
}

#[test]
fn svc_create_update_delete_lifecycle() {
    let store = new_store();
    let created = svc_create(
        &store,
        CreateRoutineRequest {
            schedule: "@daily".into(),
            title: "Cov Routine".into(),
            agent: "claude".into(),
            prompt: "p".into(),
            repositories: vec![],
            enabled: true,
        },
    )
    .unwrap();
    let id = created.routine.id.clone();
    // folder is slug of the title, not the UUID
    assert!(crate::paths::routine_toml_path("cov-routine").exists());
    assert!(crate::paths::routine_prompt_path("cov-routine").exists());

    let updated = svc_update(
        &store,
        &id,
        UpdateRoutineRequest {
            schedule: Some("@weekly".into()),
            title: Some("Renamed".into()),
            agent: Some("codex".into()),
            prompt: Some("p2".into()),
            repositories: Some(vec![Repository {
                repository: "r".into(),
                branch: None,
            }]),
            enabled: Some(false),
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
        title: Some("x".into()),
        agent: None,
        prompt: None,
        repositories: None,
        enabled: None,
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
        title: None,
        agent: None,
        prompt: None,
        repositories: None,
        enabled: None,
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
    let mut r = make_routine("trig-id");
    r.agent = "no-such-agent-xyz".into();
    store.lock().unwrap().insert("trig-id".into(), r);
    let triggered = svc_trigger(&store, "trig-id").unwrap();
    assert!(triggered.last_triggered_at.is_some());
    // folder is slug of "My Routine"
    crate::routine_storage::remove_routine_dir("my-routine").unwrap();
}

#[test]
fn load_agent_command_missing_returns_none() {
    assert!(load_agent_command("definitely-not-an-agent-zzz").is_none());
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
    let mut r = make_routine("trig-cfg");
    r.title = title.into();
    r.agent = agent_name.into();
    store.lock().unwrap().insert("trig-cfg".into(), r.clone());
    crate::routine_storage::write_routine(&r).unwrap();

    let triggered = svc_trigger(&store, "trig-cfg").unwrap();
    assert!(triggered.last_triggered_at.is_some());

    // Let the fire-and-forget shell create its workbench, then clean everything up.
    std::thread::sleep(std::time::Duration::from_millis(150));
    std::fs::remove_file(&cfg).unwrap();
    // folder is slug of title "Trigger Cov Title ZZZ"
    crate::routine_storage::remove_routine_dir("trigger-cov-title-zzz").unwrap();
    let prefix = format!("{}-", slugify(title));
    if let Ok(entries) = std::fs::read_dir(crate::paths::workbenches_dir()) {
        for e in entries.flatten() {
            if e.file_name().to_string_lossy().starts_with(&prefix) {
                let _ = std::fs::remove_dir_all(e.path());
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
    let mut r = make_routine("logs-id");
    r.title = "Unlikely Title For Logs 9988".into();
    store.lock().unwrap().insert("logs-id".into(), r);
    assert_eq!(svc_logs(&store, "logs-id").unwrap(), "");
}

#[test]
fn svc_logs_returns_newest_workbench_log() {
    let store = new_store();
    let mut r = make_routine("logs-newest");
    r.title = "Logs Cov Newest AAA".into();
    let slug = slugify(&r.title);
    store.lock().unwrap().insert("logs-newest".into(), r);

    let wb = crate::paths::workbenches_dir();
    let old = wb.join(format!("{slug}-1000"));
    let new = wb.join(format!("{slug}-2000"));
    std::fs::create_dir_all(&old).unwrap();
    std::fs::create_dir_all(&new).unwrap();
    std::fs::write(old.join("agent.log"), "old-log").unwrap();
    std::fs::write(new.join("agent.log"), "new-log").unwrap();

    assert_eq!(svc_logs(&store, "logs-newest").unwrap(), "new-log");

    std::fs::remove_dir_all(&old).unwrap();
    std::fs::remove_dir_all(&new).unwrap();
}

#[test]
fn svc_logs_empty_when_newest_has_no_log_file() {
    let store = new_store();
    let mut r = make_routine("logs-nofile");
    r.title = "Logs Cov NoFile BBB".into();
    let slug = slugify(&r.title);
    store.lock().unwrap().insert("logs-nofile".into(), r);

    let dir = crate::paths::workbenches_dir().join(format!("{slug}-3000"));
    std::fs::create_dir_all(&dir).unwrap();
    assert_eq!(svc_logs(&store, "logs-nofile").unwrap(), "");
    std::fs::remove_dir_all(&dir).unwrap();
}
