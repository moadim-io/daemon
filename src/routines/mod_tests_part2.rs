#![allow(
    clippy::wildcard_imports,
    clippy::too_many_arguments,
    clippy::needless_pass_by_value,
    dead_code,
    reason = "split-out module preserves existing code while keeping files under the linecheck limit"
)]
use super::*;

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

    // hermes default parses and passes the prompt as a one-shot argument
    let hermes: AgentCommand =
        toml::from_str(&std::fs::read_to_string(dir.join("hermes.toml")).unwrap()).unwrap();
    assert_eq!(hermes.command, "hermes");
    assert_eq!(
        hermes.args,
        vec![
            "-z".to_string(),
            "{prompt}".to_string(),
            "--ignore-rules".to_string()
        ]
    );

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
