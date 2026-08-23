//! Built-in default agent config for Hermes.

/// Registry key for this agent; also the config filename stem (`hermes.toml`).
pub const NAME: &str = "hermes";

/// Default `hermes.toml` contents, written on startup when the file is absent.
///
/// Runs Hermes headless in one-shot mode with the composed prompt passed as an argument
/// (`{prompt}`). Safe mode prevents interactive customizations and MCP startup from blocking an
/// unattended routine; the explicit model/provider keep the invocation independent of the caller's
/// interactive Hermes defaults. Users can override the file under
/// `~/.config/moadim/agents/hermes.toml` if their Hermes CLI expects a different invocation.
pub const CONFIG: &str = r#"command = "hermes"
args = ["-z", "{prompt}", "--ignore-rules", "--safe-mode", "--model", "gpt-5.6-luna", "--provider", "openai-codex"]
"#;
