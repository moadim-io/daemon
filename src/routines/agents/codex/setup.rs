//! Built-in default agent config for Codex.

/// Registry key for this agent; also the config filename stem (`codex.toml`).
pub const NAME: &str = "codex";

/// Default `codex.toml` contents, written on startup when the file is absent.
///
/// Runs `codex exec` headless with the composed prompt file passed as an argument (`{prompt_file}`).
pub const CONFIG: &str = r#"command = "codex"
args = ["exec", "{prompt_file}"]
"#;
