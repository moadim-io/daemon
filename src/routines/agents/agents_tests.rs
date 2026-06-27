#![allow(clippy::missing_docs_in_private_items)]

use super::*;

/// A unique temp directory base for agent registry tests.
fn unique_dir(tag: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("moadim-agents-{tag}-{}", uuid::Uuid::new_v4()))
}

#[test]
fn available_agents_in_falls_back_when_dir_has_no_toml() {
    // Covers the `names.is_empty()` → built-in defaults branch when the directory
    // is readable but contains no `.toml` stems.
    let dir = unique_dir("empty-readable");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("notes.txt"), "ignore me").unwrap();

    assert_eq!(
        available_agents_in(&dir),
        vec![
            "claude".to_string(),
            "codex".to_string(),
            "hermes".to_string()
        ]
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn load_agent_command_parses_a_valid_config() {
    // Happy path: a well-formed config resolves to an `AgentCommand` (Ok), unchanged from before.
    let agent_name = "load-agent-valid-zzz";
    std::fs::create_dir_all(crate::paths::agents_dir()).unwrap();
    let cfg = crate::paths::agent_toml_path(agent_name);
    std::fs::write(&cfg, "command = \"claude\"\nargs = [\"--help\"]\n").unwrap();

    let loaded = load_agent_command(agent_name).unwrap();
    assert_eq!(loaded.command, "claude");
    assert_eq!(loaded.args, vec!["--help".to_string()]);

    std::fs::remove_file(&cfg).unwrap();
}

#[test]
fn load_agent_command_reports_parse_error_for_malformed_config() {
    // A present-but-unparseable config must yield `Parse` (NOT `Missing`), carrying the toml error,
    // so callers can name the real cause instead of "config not found".
    let agent_name = "load-agent-malformed-zzz";
    std::fs::create_dir_all(crate::paths::agents_dir()).unwrap();
    let cfg = crate::paths::agent_toml_path(agent_name);
    std::fs::write(&cfg, "command = [\n").unwrap();

    match load_agent_command(agent_name) {
        Err(AgentLoadError::Parse(err)) => assert!(!err.is_empty()),
        other => panic!("expected Parse error, got {other:?}"),
    }

    std::fs::remove_file(&cfg).unwrap();
}

#[test]
fn load_agent_command_reports_missing_for_absent_config() {
    // No file on disk → `Missing` (and ONLY a genuine not-found), leaving the missing-file behavior
    // identical to before.
    assert!(matches!(
        load_agent_command("load-agent-absent-zzz"),
        Err(AgentLoadError::Missing)
    ));
}

#[test]
fn load_agent_command_reports_unreadable_for_non_not_found_io_error() {
    // A present-but-unreadable config (here: a directory at the `<name>.toml` path, which yields an
    // I/O error whose kind is NOT `NotFound`) must yield `Unreadable`, NOT `Missing` — so an
    // unreadable config is never mislabeled "config not found" and silently dropped.
    let agent_name = "load-agent-unreadable-zzz";
    std::fs::create_dir_all(crate::paths::agents_dir()).unwrap();
    let cfg = crate::paths::agent_toml_path(agent_name);
    std::fs::create_dir_all(&cfg).unwrap();

    match load_agent_command(agent_name) {
        Err(AgentLoadError::Unreadable(err)) => assert!(!err.is_empty()),
        other => panic!("expected Unreadable error, got {other:?}"),
    }

    std::fs::remove_dir_all(&cfg).unwrap();
}

#[test]
fn agent_load_error_display_distinguishes_variants() {
    // Each variant renders distinctly: missing vs. unreadable vs. malformed (the latter two carrying
    // the underlying error).
    assert_eq!(
        AgentLoadError::Missing.to_string(),
        "agent config not found"
    );
    assert_eq!(
        AgentLoadError::Unreadable("permission denied".to_string()).to_string(),
        "unreadable agent config: permission denied"
    );
    assert_eq!(
        AgentLoadError::Parse("boom".to_string()).to_string(),
        "malformed agent TOML: boom"
    );
}

#[test]
fn ensure_default_agents_seeds_into_override_home() {
    // Covers the public `ensure_default_agents` wrapper, which resolves `agents_dir()` through the
    // `MOADIM_HOME_OVERRIDE` seam and seeds the built-in configs there.
    let home = unique_dir("ensure-default");
    let previous = std::env::var_os("MOADIM_HOME_OVERRIDE");
    // SAFETY: tests run single-threaded (RUST_TEST_THREADS=1); the override is restored below.
    unsafe {
        std::env::set_var("MOADIM_HOME_OVERRIDE", &home);
    }

    ensure_default_agents();
    assert!(crate::paths::agents_dir().join("claude.toml").exists());

    // SAFETY: single-threaded harness; restore the saved value.
    unsafe {
        match previous {
            Some(value) => std::env::set_var("MOADIM_HOME_OVERRIDE", value),
            None => std::env::remove_var("MOADIM_HOME_OVERRIDE"),
        }
    }
    let _ = std::fs::remove_dir_all(&home);
}

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
    // Covers the `std::fs::write` error branch: a directory already occupies the
    // path where the first agent's `.toml` file would be written, so the write
    // fails while the loop continues to the next agent.
    let dir = unique_dir("write-fail");
    std::fs::create_dir_all(&dir).unwrap();
    // Block the claude config path with a directory so writing the file fails.
    std::fs::create_dir_all(dir.join("claude.toml")).unwrap();

    ensure_default_agents_in(&dir);

    // The blocked path remains a directory (write failed, was logged, ignored).
    assert!(dir.join("claude.toml").is_dir());
    // The loop still seeded the second agent.
    assert!(dir.join("codex.toml").is_file());

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn builtin_configs_declare_expected_instructions_file() {
    // Both built-in agents now read their project instructions from AGENTS.md, unifying the
    // moadim-managed system prompt and routine-origin disclosure onto a single file. Claude Code
    // loads AGENTS.md as a memory/context file, and AGENTS.md is the file Codex reads, so the
    // disclosure lands in the file each agent actually reads.
    let claude: AgentCommand = toml::from_str(claude_code::CONFIG).unwrap();
    assert_eq!(claude.instructions_file, "AGENTS.md");

    let codex: AgentCommand = toml::from_str(codex::CONFIG).unwrap();
    assert_eq!(codex.instructions_file, "AGENTS.md");
}

#[test]
fn default_instructions_file_falls_back_to_claude_md() {
    // A config that omits `instructions_file` falls back to the historical CLAUDE.md default,
    // preserving backward compatibility for user-authored agent configs.
    let agent: AgentCommand = toml::from_str(r#"command = "x""#).unwrap();
    assert_eq!(agent.instructions_file, DEFAULT_INSTRUCTIONS_FILE);
    assert_eq!(agent.instructions_file, "CLAUDE.md");
}

#[cfg(unix)]
#[test]
fn ensure_default_agents_in_swallows_per_config_write_errors() {
    use std::os::unix::fs::PermissionsExt as _;

    // Covers the per-config `std::fs::write` error arm: the directory exists (so `create_dir_all`
    // succeeds) but is read-only, so each config write fails and is logged rather than panicking.
    let dir = unique_dir("write-fail");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o555)).unwrap();

    ensure_default_agents_in(&dir);

    // Restore permissions so cleanup can proceed. (Root bypasses the read-only bit, in which case
    // the writes succeed; the call is exercised either way.)
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
}
