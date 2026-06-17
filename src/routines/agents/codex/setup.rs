//! Built-in default agent config for Codex.

/// Registry key for this agent; also the config filename stem (`codex.toml`).
pub const NAME: &str = "codex";

/// Default `codex.toml` contents, written on startup when the file is absent.
///
/// Runs `codex exec` headless with the composed prompt file passed as an argument (`{prompt_file}`).
/// Codex reads project instructions from `AGENTS.md`, not Claude's `CLAUDE.md`, so the
/// moadim-managed system prompt and routine-origin disclosure are written there for this agent.
pub const CONFIG: &str = r#"command = "codex"
args = ["exec", "{prompt_file}"]
instructions_file = "AGENTS.md"
"#;
