//! Built-in default agent config for Hermes.

/// Registry key for this agent; also the config filename stem (`hermes.toml`).
pub const NAME: &str = "hermes";

/// Default `hermes.toml` contents, written on startup when the file is absent.
///
/// Runs `hermes exec` headless with the composed prompt file passed as an argument
/// (`{prompt_file}`), mirroring the Codex default.
pub const CONFIG: &str = r#"command = "hermes"
args = ["exec", "{prompt_file}"]
"#;
