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
        vec!["claude".to_string(), "codex".to_string()]
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn ensure_default_agents_seeds_into_override_home() {
    // Covers the public `ensure_default_agents()` wrapper by redirecting the
    // resolved home directory to a tempdir via the `MOADIM_HOME_OVERRIDE` seam.
    let home = unique_dir("override-home");
    std::fs::create_dir_all(&home).unwrap();
    // SAFETY: tests in this crate run single-threaded per binary; we set and
    // immediately restore the override around this call.
    let previous = std::env::var_os("MOADIM_HOME_OVERRIDE");
    unsafe {
        std::env::set_var("MOADIM_HOME_OVERRIDE", &home);
    }

    ensure_default_agents();

    let seeded = available_agents();
    assert!(seeded.contains(&"claude".to_string()));
    assert!(seeded.contains(&"codex".to_string()));

    unsafe {
        match previous {
            Some(value) => std::env::set_var("MOADIM_HOME_OVERRIDE", value),
            None => std::env::remove_var("MOADIM_HOME_OVERRIDE"),
        }
    }
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn ensure_default_agents_in_logs_and_returns_on_create_dir_failure() {
    // Covers the `create_dir_all` error branch: the target's parent is a regular
    // file, so creating the directory underneath it fails.
    let base = unique_dir("create-fail");
    std::fs::create_dir_all(&base).unwrap();
    let blocking_file = base.join("blocker");
    std::fs::write(&blocking_file, "i am a file, not a dir").unwrap();
    // `blocking_file` is a file, so treating it as a parent directory must fail.
    let target = blocking_file.join("agents");

    ensure_default_agents_in(&target);

    // Nothing was seeded because the directory could not be created.
    assert!(!target.exists());

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
fn ensure_default_agents_in_logs_on_write_failure_into_readonly_dir() {
    // Covers the `if let Err(err) = std::fs::write(..)` warn branch directly: the
    // agents dir exists but is read-only, so `path.exists()` is false (no file yet)
    // and the subsequent `std::fs::write` of each agent's `.toml` fails with EACCES.
    // The previous write-failure test instead blocks the path with a directory, which
    // makes `path.exists()` true and takes the `continue` arm — so it never reaches
    // the write call. This one reaches and fails the write.
    use std::os::unix::fs::PermissionsExt as _;

    let dir = unique_dir("write-fail-readonly");
    std::fs::create_dir_all(&dir).unwrap();
    // Read+execute but NOT write: file creation inside is denied.
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o555)).unwrap();

    ensure_default_agents_in(&dir);

    // Restore permissions so the dir can be inspected and cleaned up.
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
    // Nothing could be written: every agent write failed and was only logged.
    assert!(!dir.join("claude.toml").exists());
    assert!(!dir.join("codex.toml").exists());

    let _ = std::fs::remove_dir_all(&dir);
}
