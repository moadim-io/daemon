#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

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
            "hermes".to_string(),
            "pi".to_string()
        ]
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn codex_default_config_enables_sandbox_network_access() {
    // `codex exec`'s default workspace-write sandbox disables outbound network, which would block
    // an unattended routine from cloning the repo or pushing / opening a PR. The shipped default
    // must re-enable it; parse the actual config string and assert the override is present so the
    // flag can't silently regress to the network-disabled default.
    let cmd: AgentCommand =
        toml::from_str(super::codex::CONFIG).expect("codex default config must be valid TOML");
    assert_eq!(cmd.command, "codex");
    assert!(
        cmd.args
            .iter()
            .any(|arg| arg == "sandbox_workspace_write.network_access=true"),
        "codex default args must enable sandbox network access, got {:?}",
        cmd.args
    );
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

// ── ensure_default_agents_in: reconciliation of an existing config (#428) ───────────────────────

#[test]
fn ensure_default_agents_in_seeds_with_a_managed_fingerprint_header() {
    // Absent -> seeded case (unchanged from before), now also asserting the seeded file carries
    // the managed header this reconciliation is built on.
    let dir = unique_dir("seed-header");
    std::fs::create_dir_all(&dir).unwrap();

    ensure_default_agents_in(&dir);

    let written = std::fs::read_to_string(dir.join("claude.toml")).unwrap();
    let (hash, body) = parse_managed(&written).expect("seeded file must carry the managed header");
    assert_eq!(body, claude_code::CONFIG);
    assert_eq!(hash, fingerprint(claude_code::CONFIG));

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn ensure_default_agents_in_is_a_noop_when_already_current() {
    // A managed file whose body already equals the current built-in must not be rewritten.
    let dir = unique_dir("already-current");
    std::fs::create_dir_all(&dir).unwrap();
    ensure_default_agents_in(&dir);
    let before = std::fs::read_to_string(dir.join("claude.toml")).unwrap();

    ensure_default_agents_in(&dir);

    let after = std::fs::read_to_string(dir.join("claude.toml")).unwrap();
    assert_eq!(before, after);

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn ensure_default_agents_in_upgrades_a_stale_pristine_config() {
    // (a) stale-pristine -> upgraded: a managed file whose body is an old built-in (never edited,
    // as proven by its body still hashing to its own recorded fingerprint) must be refreshed to the
    // current built-in, with a fingerprint recorded for it in turn.
    let dir = unique_dir("stale-pristine");
    std::fs::create_dir_all(&dir).unwrap();
    let stale_body = "command = \"old-claude\"\n";
    std::fs::write(dir.join("claude.toml"), render_managed(stale_body)).unwrap();

    ensure_default_agents_in(&dir);

    let upgraded = std::fs::read_to_string(dir.join("claude.toml")).unwrap();
    let (hash, body) = parse_managed(&upgraded).expect("still managed after upgrade");
    assert_eq!(body, claude_code::CONFIG);
    assert_eq!(hash, fingerprint(claude_code::CONFIG));

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn ensure_default_agents_in_preserves_a_user_edited_config() {
    // (b) user-edited -> preserved: a managed file whose current body no longer hashes to its
    // recorded fingerprint (the user changed it after the daemon wrote it) must never be touched,
    // even though its body also differs from the current built-in.
    let dir = unique_dir("user-edited");
    std::fs::create_dir_all(&dir).unwrap();
    let managed = render_managed("command = \"old-claude\"\n");
    let edited = format!("{managed}\n# a user note appended after seeding\n");
    std::fs::write(dir.join("claude.toml"), &edited).unwrap();

    ensure_default_agents_in(&dir);

    assert_eq!(
        std::fs::read_to_string(dir.join("claude.toml")).unwrap(),
        edited,
        "a user-edited config must never be overwritten"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn ensure_default_agents_in_leaves_a_legacy_unmanaged_config_untouched() {
    // A config with no managed header at all (seeded before this mechanism existed, or hand-authored)
    // has no provenance to trust, so it must be left strictly alone rather than guessed at.
    let dir = unique_dir("legacy-no-header");
    std::fs::create_dir_all(&dir).unwrap();
    let legacy = "command = \"legacy-claude\"\n";
    std::fs::write(dir.join("claude.toml"), legacy).unwrap();

    ensure_default_agents_in(&dir);

    assert_eq!(
        std::fs::read_to_string(dir.join("claude.toml")).unwrap(),
        legacy
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn ensure_default_agents_in_leaves_a_malformed_managed_header_untouched() {
    // Covers `parse_managed`'s `split_once` returning `None`: the header prefix is present but
    // there's no trailing newline/body, so the file can't be trusted as managed and is left alone.
    let dir = unique_dir("malformed-header");
    std::fs::create_dir_all(&dir).unwrap();
    let malformed = format!("{MANAGED_HEADER_PREFIX}deadbeef");
    std::fs::write(dir.join("claude.toml"), &malformed).unwrap();

    ensure_default_agents_in(&dir);

    assert_eq!(
        std::fs::read_to_string(dir.join("claude.toml")).unwrap(),
        malformed
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[cfg(unix)]
#[test]
fn ensure_default_agents_in_logs_when_rewriting_a_stale_pristine_config_fails() {
    use std::os::unix::fs::PermissionsExt as _;

    // Covers the rewrite-failure arm, distinct from the seed-failure arm covered by
    // `ensure_default_agents_in_swallows_per_config_write_errors`: here the file already exists (a
    // pristine-but-stale managed config) and its own permissions block the overwrite.
    let dir = unique_dir("stale-write-fail");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("claude.toml");
    std::fs::write(&path, render_managed("command = \"old-claude\"\n")).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o444)).unwrap();

    ensure_default_agents_in(&dir);

    // Restore permissions so cleanup can proceed. (Root bypasses the read-only bit, in which case
    // the rewrite succeeds; the call is exercised either way.)
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
}

// ── available_agents_in: extension-filter branch ────────────────────────────

#[test]
fn available_agents_in_ignores_files_without_extension() {
    // Exercises the `path.extension()?` → None branch in the filter_map closure:
    // a file with no extension (e.g. "readme") has extension() == None, so the `?`
    // propagates None and the entry is skipped.  The .toml files are still returned.
    let dir = unique_dir("no-ext");
    std::fs::create_dir_all(&dir).unwrap();
    // A file without any extension — must be ignored.
    std::fs::write(dir.join("readme"), "not an agent").unwrap();
    // A valid agent config — must be returned.
    std::fs::write(dir.join("my-agent.toml"), "command = \"x\"\n").unwrap();

    let agents = available_agents_in(&dir);
    assert!(
        agents.contains(&"my-agent".to_string()),
        "toml file should be listed"
    );
    assert!(
        !agents.iter().any(|n| n == "readme"),
        "no-extension file must be filtered out"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn available_agents_in_ignores_files_with_non_toml_extension() {
    // Extension is Some but != "toml": the entry is filtered out.
    let dir = unique_dir("non-toml-ext");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("notes.txt"), "ignore").unwrap();
    std::fs::write(dir.join("claude.toml"), "command = \"claude\"\n").unwrap();

    let agents = available_agents_in(&dir);
    assert_eq!(agents, vec!["claude".to_string()]);

    let _ = std::fs::remove_dir_all(&dir);
}
