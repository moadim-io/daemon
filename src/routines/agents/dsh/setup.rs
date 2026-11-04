//! Built-in default agent config for `dsh` (`DeepSeek` Harness).

/// Registry key for this agent; also the config filename stem (`dsh.toml`).
pub const NAME: &str = "dsh";

/// Default `dsh.toml` contents, written on startup when the file is absent.
///
/// Boots dsh's `headless` profile, which answers one task, prints the result and exits — no
/// server, no browser. The composed prompt goes in as one argument (`{prompt}`). dsh loads
/// `AGENTS.md` from the working directory as project instructions, so the moadim system prompt is
/// written there.
pub const CONFIG: &str = r#"command = "dsh"
args = ["--profile", "headless", "{prompt}"]
instructions_file = "AGENTS.md"
"#;
