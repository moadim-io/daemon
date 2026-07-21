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
fn slugify_preserves_non_ascii_letters() {
    // Hebrew and CJK titles must not collapse to the "routine" fallback (#262).
    assert_eq!(slugify("עדכון יומי"), "עדכון-יומי");
    assert_eq!(slugify("日次レポート"), "日次レポート");
    assert_eq!(slugify("Отчёт"), "отчёт");
    // Latin diacritics are kept rather than silently dropped.
    assert_eq!(slugify("Café Report"), "café-report");
}

#[test]
fn slugify_distinct_non_ascii_titles_produce_distinct_slugs() {
    let slug_one = slugify("עדכון יומי");
    let slug_two = slugify("דוח שבועי");
    assert_ne!(slug_one, "routine");
    assert_ne!(slug_two, "routine");
    assert_ne!(slug_one, slug_two);
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
fn compose_prompt_without_repositories_omits_clone_header() {
    let mut routine = make_routine("x");
    routine.repositories = vec![];
    let prompt = compose_prompt(&routine);
    assert!(prompt.contains("# Workbench"));
    assert!(prompt.contains("You are working in an empty directory.\n"));
    // No dangling "clone any you need:" header (and no empty bullet list) when there are no repos.
    assert!(!prompt.contains("clone any you need"));
    assert!(!prompt.contains("\n- "));
    assert!(prompt.contains("do the thing"));
}

#[test]
fn compose_prompt_renders_goal_section_when_set() {
    let mut routine = make_routine("x");
    routine.goal = Some("Keep the PR backlog small.".to_string());
    let prompt = compose_prompt(&routine);
    // The goal appears as a `## Goal` section before the `---` prompt separator.
    let goal_at = prompt.find("## Goal").expect("goal section present");
    let sep_at = prompt.find("\n---\n").expect("prompt separator present");
    assert!(goal_at < sep_at, "goal must precede the prompt");
    assert!(prompt.contains("Keep the PR backlog small."));
}

#[test]
fn compose_prompt_omits_goal_section_when_unset_or_blank() {
    let mut routine = make_routine("x");
    routine.goal = None;
    assert!(!compose_prompt(&routine).contains("## Goal"));
    routine.goal = Some("   \n\t".to_string());
    assert!(!compose_prompt(&routine).contains("## Goal"));
}

#[test]
fn compose_prompt_omits_open_flags_section_when_none() {
    let routine = make_routine("x");
    let prompt = compose_prompt(&routine);
    assert!(!prompt.contains("Open flags"));
}

#[test]
fn compose_prompt_includes_open_flags_section() {
    let mut routine = make_routine("x");
    routine.title = "Compose Prompt Flags Test ZZZ".to_string();
    let slug = slugify(&routine.title);
    flags::create_flag(
        &slug,
        "bug",
        "the thing is broken",
        flags::FlagScope::General,
    )
    .unwrap();
    flags::create_flag(&slug, "gap", "missing context", flags::FlagScope::Local).unwrap();

    let prompt = compose_prompt(&routine);
    assert!(prompt.contains("# Open flags"));
    assert!(prompt.contains("**bug** (general): the thing is broken"));
    assert!(prompt.contains("**gap** (local): missing context"));

    crate::routine_storage::remove_routine_dir(&slug).unwrap();
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
        instructions_file: "CLAUDE.md".to_string(),
        setup: None,
    };
    let cmd = build_routine_command(&routine, &agent, TriggerSource::Scheduled);
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
    // pipe-pane is chained onto the same tmux invocation as new-session (#289), not a
    // standalone `tmux pipe-pane` statement.
    assert!(cmd.contains(r#"\; pipe-pane -o -t "$SESS""#));
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
        instructions_file: "AGENTS.md".to_string(),
        setup: None,
    };
    let cmd = build_routine_command(&routine, &agent, TriggerSource::Scheduled);
    // The invocation is quoted as one `tmux new-session` shell-command argument together with
    // the exit-code capture appended to it (see `build_routine_command_records_exit_code_after_invocation`
    // in `command_tests.rs`), so the substituted invocation no longer stands alone as its own
    // quoted string.
    assert!(cmd.contains("codex exec prompt.md;"));
}

#[test]
fn build_routine_command_writes_claude_md() {
    let routine = make_routine("rid");
    let agent = AgentCommand {
        command: "claude".to_string(),
        args: vec!["{prompt}".to_string()],
        instructions_file: "CLAUDE.md".to_string(),
        setup: None,
    };
    let cmd = build_routine_command(&routine, &agent, TriggerSource::Scheduled);
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
        "CLAUDE.md write should precede prompt copy"
    );
}

#[test]
fn compose_prompt_writes_routine_origin_disclosure() {
    let routine = make_routine("rid");
    let prompt = compose_prompt(&routine);
    assert!(prompt.contains("Routine origin disclosure"));
    assert!(prompt.contains("Routine name: My Routine"));
}

#[test]
fn build_routine_command_writes_disclosure_to_codex_instructions_file() {
    // Codex reads project instructions from AGENTS.md, not CLAUDE.md. The daemon-managed system
    // prompt must land in the file the selected agent actually reads.
    let routine = make_routine("rid");
    let agent = AgentCommand {
        command: "codex".to_string(),
        args: vec!["exec".to_string(), "{prompt_file}".to_string()],
        instructions_file: "AGENTS.md".to_string(),
        setup: None,
    };
    let cmd = build_routine_command(&routine, &agent, TriggerSource::Scheduled);
    // The prompt file is still written to AGENTS.md, the file Codex reads...
    assert!(
        cmd.contains(r#"> "$WB/AGENTS.md""#),
        "moadim prompt should be written to AGENTS.md for the codex agent"
    );
    assert!(
        cmd.contains(r#">> "$WB/AGENTS.md""#),
        "user prompt should be appended to AGENTS.md for the codex agent"
    );
    // ...and the disclosure now lives in the compiled prompt body.
    assert!(
        compose_prompt(&routine).contains("Routine origin disclosure"),
        "routine-origin disclosure section missing from compiled prompt"
    );
    // CLAUDE.md is not written for a codex routine: Codex would never read it.
    assert!(
        !cmd.contains("CLAUDE.md"),
        "codex routine must not write the Claude-only CLAUDE.md"
    );
}

#[test]
fn build_routine_command_aborts_when_prompt_missing() {
    let routine = make_routine("rid");
    let agent = AgentCommand {
        command: "claude".to_string(),
        args: vec!["{prompt}".to_string()],
        instructions_file: "CLAUDE.md".to_string(),
        setup: None,
    };
    let cmd = build_routine_command(&routine, &agent, TriggerSource::Scheduled);
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
        instructions_file: "CLAUDE.md".to_string(),
        setup: Some("seed-trust \"$WB\"".to_string()),
    };
    let cmd = build_routine_command(&routine, &agent, TriggerSource::Scheduled);
    let setup_at = cmd.find("seed-trust").expect("setup present");
    let launch_at = cmd.find("tmux new-session").expect("launch present");
    // setup runs before the agent launches
    assert!(setup_at < launch_at);
    // inserted verbatim (not shell-quoted), $WB left for the runtime shell to expand
    assert!(cmd.contains("seed-trust \"$WB\""));
}

#[test]
fn build_routine_command_redirects_launch_wrapper_to_launch_log() {
    // Setup/tmux failures must not be silently mailed by cron on a headless host (#375): everything
    // from the prompt copy through the chained `pipe-pane` (#289) runs inside a
    // `{ … } >> "$WB/launch.log" 2>&1` group, so a failure anywhere in that wrapper leaves a
    // readable trace in the workbench.
    let routine = make_routine("rid");
    let agent = AgentCommand {
        command: "claude".to_string(),
        args: vec!["{prompt}".to_string()],
        instructions_file: "CLAUDE.md".to_string(),
        setup: Some("seed-trust \"$WB\"".to_string()),
    };
    let cmd = build_routine_command(&routine, &agent, TriggerSource::Scheduled);
    assert!(
        cmd.contains(r#"} >> "$WB/launch.log" 2>&1"#),
        "expected the setup/launch wrapper to redirect into launch.log in: {cmd}"
    );

    // The redirect group opens after `mkdir -p "$WB"` (so $WB exists before anything tries to
    // write into it) and closes after the final (chained) `pipe-pane` statement.
    let mkdir_at = cmd.find(r#"mkdir -p "$WB""#).expect("mkdir present");
    let group_open_at = cmd[mkdir_at..].find('{').map(|off| mkdir_at + off).unwrap();
    let setup_at = cmd.find("seed-trust").expect("setup present");
    let pipe_pane_at = cmd
        .find(r#"\; pipe-pane -o -t "$SESS""#)
        .expect("pipe-pane present");
    let redirect_at = cmd.find(r#"} >> "$WB/launch.log""#).unwrap();
    assert!(
        mkdir_at < group_open_at,
        "mkdir must run before the redirected group opens"
    );
    assert!(
        group_open_at < setup_at,
        "setup must run inside the redirected group"
    );
    assert!(
        pipe_pane_at < redirect_at,
        "pipe-pane must run inside the redirected group"
    );
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

    // pi default parses and runs print mode against the composed prompt file
    let pi: AgentCommand =
        toml::from_str(&std::fs::read_to_string(dir.join("pi.toml")).unwrap()).unwrap();
    assert_eq!(pi.command, "pi");
    assert!(pi.args.contains(&"--approve".to_string()));
    assert!(pi.args.contains(&"-p".to_string()));
    assert!(pi.args.contains(&"@{prompt_file}".to_string()));

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn ensure_default_agents_does_not_overwrite_existing() {
    let dir = std::env::temp_dir().join("moadim-agents-preserve-test");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("claude.toml"), "command = \"mine\"\nargs = []\n").unwrap();

    ensure_default_agents_in(&dir);

    // user file untouched, built-in defaults still seeded
    assert_eq!(
        std::fs::read_to_string(dir.join("claude.toml")).unwrap(),
        "command = \"mine\"\nargs = []\n"
    );
    assert!(dir.join("codex.toml").exists());
    assert!(dir.join("pi.toml").exists());

    let _ = std::fs::remove_dir_all(&dir);
}
