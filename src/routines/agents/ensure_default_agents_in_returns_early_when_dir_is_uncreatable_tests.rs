
#[test]
fn ensure_default_agents_in_returns_early_when_dir_is_uncreatable() {
    // Covers the `create_dir_all` error arm: a path whose parent is a regular file can never be
    // created, so the function logs and returns without writing any config.
    let base = unique_dir("uncreatable");
    std::fs::create_dir_all(&base).unwrap();
    let file = base.join("iamafile");
    std::fs::write(&file, "x").unwrap();
    let unmakeable = file.join("sub"); // parent is a file -> create_dir_all errors

    ensure_default_agents_in(&unmakeable);
    assert!(!unmakeable.exists());

    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn ensure_default_agents_in_logs_and_continues_on_write_failure() {
    // Covers the `std::fs::read_to_string` non-`NotFound`-error branch: a directory already
    // occupies the path where the first agent's `.toml` file would live, so reading it fails (it's
    // a directory, not a file) and is logged while the loop continues to the next agent.
    let dir = unique_dir("write-fail");
    std::fs::create_dir_all(&dir).unwrap();
    // Block the claude config path with a directory so reading it fails.
    std::fs::create_dir_all(dir.join("claude.toml")).unwrap();

    ensure_default_agents_in(&dir);

    // The blocked path remains a directory (read failed, was logged, ignored).
    assert!(dir.join("claude.toml").is_dir());
    // The loop still seeded the second agent.
    assert!(dir.join("codex.toml").is_file());

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn builtin_configs_declare_expected_instructions_file() {
    // Claude Code loads AGENTS.md as a memory/context file, and AGENTS.md is the file Codex
    // reads, so the disclosure lands in the file each agent actually reads.
    let claude: AgentCommand = toml::from_str(claude_code::CONFIG).unwrap();
    assert_eq!(claude.instructions_file, "AGENTS.md");

    let codex: AgentCommand = toml::from_str(codex::CONFIG).unwrap();
    assert_eq!(codex.instructions_file, "AGENTS.md");
}

#[test]
fn pi_default_config_parses_and_uses_prompt_file() {
    // Pi runs one-shot in print mode here, with the composed prompt file attached and project
    // trust approved so unattended routines do not stall on a prompt.
    let pi: AgentCommand = toml::from_str(super::pi::CONFIG).unwrap();
    assert_eq!(pi.command, "pi");
    assert!(pi.args.contains(&"--approve".to_string()));
    assert!(pi.args.contains(&"-p".to_string()));
    assert!(pi.args.contains(&"@{prompt_file}".to_string()));
    assert_eq!(pi.instructions_file, DEFAULT_INSTRUCTIONS_FILE);
}

#[test]
fn default_instructions_file_falls_back_to_claude_md() {
    // A config that omits `instructions_file` falls back to the historical CLAUDE.md default,
    // preserving backward compatibility for user-authored agent configs.
    let agent: AgentCommand = toml::from_str(r#"command = "x""#).unwrap();
    assert_eq!(agent.instructions_file, DEFAULT_INSTRUCTIONS_FILE);
    assert_eq!(agent.instructions_file, "CLAUDE.md");
}

// ── ensure_default_agents_in: reconciliation of an existing config (#428) ───────────────────────

// ── available_agents_in: extension-filter branch ────────────────────────────

#[allow(
    clippy::missing_docs_in_private_items,
    reason = "split-out module keeps the file under the linecheck limit"
)]
#[path = "ensure_default_agents_in_swallows_per_config_write_errors_tests.rs"]
mod ensure_default_agents_in_swallows_per_config_write_errors_tests;
