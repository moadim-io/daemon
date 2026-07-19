//! Built-in default agent config for Pi.

/// Registry key for this agent; also the config filename stem (`pi.toml`).
pub const NAME: &str = "pi";

/// Default `pi.toml` contents, written on startup when the file is absent.
///
/// Runs Pi in print mode with the composed prompt file attached, so the daemon-rendered
/// `CLAUDE.md` context is loaded and the run exits after one unattended response. `--approve`
/// skips the interactive trust prompt and lets the run use project-local resources for this run.
pub const CONFIG: &str = r#"command = "pi"
args = ["--approve", "-p", "@{prompt_file}"]
"#;
