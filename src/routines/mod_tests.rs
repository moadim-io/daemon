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
        ttl_secs: None,
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
    let routine = make_routine("x");
    let prompt = compose_prompt(&routine);
    assert!(prompt.contains("# Workbench"));
    assert!(prompt.contains("https://github.com/octocat/Hello-World (branch master)"));
    assert!(prompt.contains("do the thing"));
}

#[test]
fn compose_prompt_repo_without_branch() {
    let mut routine = make_routine("x");
    routine.repositories = vec![Repository {
        repository: "git@example.com:a/b".to_string(),
        branch: None,
    }];
    let prompt = compose_prompt(&routine);
    assert!(prompt.contains("- git@example.com:a/b\n"));
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
    let routine = make_routine("rid");
    let agent = AgentCommand {
        command: "claude".to_string(),
        args: vec![
            "--dangerously-skip-permissions".to_string(),
            "{prompt}".to_string(),
        ],
        setup: None,
    };
    let cmd = build_routine_command(&routine, &agent);
    assert!(cmd.contains("tmux new-session -d -s \"$SESS\" -c \"$WB\""));
    // bakes a PATH export so cron's minimal PATH does not hide tmux/claude
    assert!(cmd.contains("export PATH="));
    // sanity-check: command must stay in a reasonable range; the PATH export and system-prompt
    // setup add several hundred chars, so the limit is higher than the raw cron-line minimum
    assert!(
        cmd.len() < 3000,
        "crontab line unexpectedly long: {} chars",
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
    let routine = make_routine("rid");
    let agent = AgentCommand {
        command: "codex".to_string(),
        args: vec!["exec".to_string(), "{prompt_file}".to_string()],
        setup: None,
    };
    let cmd = build_routine_command(&routine, &agent);
    assert!(cmd.contains("'codex exec prompt.md'"));
}

#[test]
fn build_routine_command_writes_claude_md() {
    let routine = make_routine("rid");
    let agent = AgentCommand {
        command: "claude".to_string(),
        args: vec!["{prompt}".to_string()],
        setup: None,
    };
    let cmd = build_routine_command(&routine, &agent);
    // moadim-managed section written via printf %b
    assert!(cmd.contains("CLAUDE.md"), "CLAUDE.md write missing");
    assert!(
        cmd.contains("Moadim Context"),
        "moadim system prompt header missing"
    );
    // dynamic date/timezone appended at run time
    assert!(cmd.contains("$(date)"), "run-time date expansion missing");
    // user prompt appended if file exists
    assert!(
        cmd.contains("user_prompt.md"),
        "user_prompt.md reference missing"
    );
    // CLAUDE.md written before cp prompt.md so both files land in $WB before agent launch
    let claude_md_at = cmd.find("CLAUDE.md").expect("CLAUDE.md in cmd");
    let prompt_md_at = cmd.find("cp ").expect("cp in cmd");
    assert!(
        claude_md_at < prompt_md_at,
        "CLAUDE.md write should precede cp prompt.md"
    );
}

#[test]
fn build_routine_command_aborts_when_prompt_missing() {
    let routine = make_routine("rid");
    let agent = AgentCommand {
        command: "claude".to_string(),
        args: vec!["{prompt}".to_string()],
        setup: None,
    };
    let cmd = build_routine_command(&routine, &agent);
    // The cp of the routine's source prompt must fail-fast: a missing source aborts the launch
    // instead of starting the agent with an empty "$(cat prompt.md)" argument (a task-less session).
    let cp_at = cmd.find("cp ").expect("cp in cmd");
    let abort_at = cmd[cp_at..]
        .find("exit 1")
        .map(|off| cp_at + off)
        .expect("cp should be guarded by an abort");
    let launch_at = cmd.find("tmux new-session").expect("launch present");
    assert!(
        abort_at < launch_at,
        "cp abort guard must precede the agent launch"
    );
    // failure reason is recorded in the workbench agent.log
    assert!(cmd[cp_at..].contains(r#""$WB/agent.log""#));
}

#[test]
fn build_routine_command_inserts_setup_before_launch() {
    let routine = make_routine("rid");
    let agent = AgentCommand {
        command: "claude".to_string(),
        args: vec!["{prompt}".to_string()],
        setup: Some("seed-trust \"$WB\"".to_string()),
    };
    let cmd = build_routine_command(&routine, &agent);
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

    // hermes default parses and passes the prompt file as an argument
    let hermes: AgentCommand =
        toml::from_str(&std::fs::read_to_string(dir.join("hermes.toml")).unwrap()).unwrap();
    assert_eq!(hermes.command, "hermes");
    assert!(hermes.args.contains(&"{prompt_file}".to_string()));

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
fn svc_get_not_found() {
    assert!(svc_get(&new_store(), "missing").is_err());
}

#[test]
fn svc_list_empty() {
    assert!(svc_list(&new_store(), &RoutineListQuery::default()).is_empty());
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
    let list = svc_list(&store, &RoutineListQuery::default());
    assert_eq!(list[0].routine.id, "early");
    assert_eq!(list[1].routine.id, "late");
}

#[test]
fn svc_list_descending_order() {
    let store = new_store();
    let mut early = make_routine("early");
    early.created_at = 10;
    let mut late = make_routine("late");
    late.created_at = 20;
    store.lock().unwrap().insert("early".into(), early);
    store.lock().unwrap().insert("late".into(), late);
    let query = RoutineListQuery {
        order: SortOrder::Desc,
        ..Default::default()
    };
    let list = svc_list(&store, &query);
    assert_eq!(list[0].routine.id, "late");
    assert_eq!(list[1].routine.id, "early");
}

#[test]
fn svc_list_filters_by_repository_substring() {
    let store = new_store();
    let mut alpha = make_routine("alpha");
    alpha.repositories = vec![Repository {
        repository: "https://github.com/octocat/Alpha".to_string(),
        branch: None,
    }];
    let mut beta = make_routine("beta");
    beta.repositories = vec![Repository {
        repository: "https://github.com/octocat/Beta".to_string(),
        branch: None,
    }];
    store.lock().unwrap().insert("alpha".into(), alpha);
    store.lock().unwrap().insert("beta".into(), beta);
    let query = RoutineListQuery {
        // Case-insensitive substring match.
        repository: Some("alpha".to_string()),
        ..Default::default()
    };
    let list = svc_list(&store, &query);
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].routine.id, "alpha");
}

#[test]
fn svc_list_sorts_by_repository_no_repo_last() {
    let store = new_store();
    let mut zeta = make_routine("zeta");
    zeta.repositories = vec![Repository {
        repository: "https://github.com/octocat/Zeta".to_string(),
        branch: None,
    }];
    let mut apple = make_routine("apple");
    apple.repositories = vec![Repository {
        repository: "https://github.com/octocat/Apple".to_string(),
        branch: None,
    }];
    let mut none = make_routine("none");
    none.repositories = vec![];
    store.lock().unwrap().insert("zeta".into(), zeta);
    store.lock().unwrap().insert("apple".into(), apple);
    store.lock().unwrap().insert("none".into(), none);
    let query = RoutineListQuery {
        sort: RoutineSort::Repository,
        ..Default::default()
    };
    let list = svc_list(&store, &query);
    assert_eq!(list[0].routine.id, "apple");
    assert_eq!(list[1].routine.id, "zeta");
    // Routines with no repository sort last.
    assert_eq!(list[2].routine.id, "none");
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
        ttl_secs: None,
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
            ttl_secs: None,
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
            ttl_secs: None,
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
        ttl_secs: None,
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
        ttl_secs: None,
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
    let mut routine = make_routine("trig-cfg");
    routine.title = title.into();
    routine.agent = agent_name.into();
    store
        .lock()
        .unwrap()
        .insert("trig-cfg".into(), routine.clone());
    crate::routine_storage::write_routine(&routine).unwrap();

    let triggered = svc_trigger(&store, "trig-cfg").unwrap();
    assert!(triggered.last_triggered_at.is_some());

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
    assert_eq!(svc_logs(&store, "logs-id").unwrap(), "");
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

    assert_eq!(svc_logs(&store, "logs-newest").unwrap(), "new-log");

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
    assert_eq!(svc_logs(&store, "logs-nofile").unwrap(), "");
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

    assert_eq!(svc_logs(&store, "logs-prefix").unwrap(), "mine");

    std::fs::remove_dir_all(&mine).unwrap();
    std::fs::remove_dir_all(&other).unwrap();
}
