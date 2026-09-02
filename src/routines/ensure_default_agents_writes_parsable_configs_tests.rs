
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
            "--ignore-rules".to_string(),
            "--safe-mode".to_string()
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
