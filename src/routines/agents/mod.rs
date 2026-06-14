//! The agent registry: resolving `~/.config/moadim/agents/<name>.toml` and seeding built-in defaults.
//!
//! Each supported agent contributes its registry key and default config from its own
//! `<agent>/setup.rs` module; [`DEFAULT_AGENT_CONFIGS`] assembles them for [`ensure_default_agents`].

use serde::Deserialize;
use std::path::Path;

use crate::paths::{agent_toml_path, agents_dir};

#[path = "claude_code/setup.rs"]
mod claude_code;
#[path = "codex/setup.rs"]
mod codex;

/// A resolved agent invocation read from `~/.config/moadim/agents/<name>.toml`.
#[derive(Debug, Clone, Deserialize)]
pub struct AgentCommand {
    /// Executable to run (e.g. `"claude"`).
    pub command: String,
    /// Arguments passed to the executable. Supports `{workbench}`, `{prompt_file}`, and `{prompt}`
    /// placeholders; `{prompt}` inlines the composed prompt as a single shell-quoted argument.
    #[serde(default)]
    pub args: Vec<String>,
    /// Optional shell command run in the workbench *before* the agent launches, inserted verbatim
    /// into the cron line. Runs with the shell vars `$WB` (absolute workbench path) and `$SESS`
    /// (tmux session name) in scope — e.g. to pre-seed per-directory editor trust state.
    #[serde(default)]
    pub setup: Option<String>,
}

/// Load the agent command for `name`, returning `None` if the config is missing or invalid.
pub fn load_agent_command(name: &str) -> Option<AgentCommand> {
    let text = std::fs::read_to_string(agent_toml_path(name)).ok()?;
    toml::from_str(&text).ok()
}

/// Built-in default agent configs `(name, toml)`, written on startup if the file does not exist.
const DEFAULT_AGENT_CONFIGS: &[(&str, &str)] = &[
    (claude_code::NAME, claude_code::CONFIG),
    (codex::NAME, codex::CONFIG),
];

/// Write any missing built-in agent configs into `~/.config/moadim/agents/`.
///
/// Existing files are never overwritten, so user edits are preserved. Best-effort: directory or
/// write failures are logged and ignored rather than aborting startup.
pub fn ensure_default_agents() {
    ensure_default_agents_in(&agents_dir());
}

/// Write missing built-in agent configs into `dir`. See [`ensure_default_agents`].
pub(crate) fn ensure_default_agents_in(dir: &Path) {
    if let Err(e) = std::fs::create_dir_all(dir) {
        log::warn!("ensure_default_agents: failed to create {dir:?}: {e}");
        return;
    }
    for (name, contents) in DEFAULT_AGENT_CONFIGS {
        let path = dir.join(format!("{name}.toml"));
        if path.exists() {
            continue;
        }
        if let Err(e) = std::fs::write(&path, contents) {
            log::warn!("ensure_default_agents: failed to write {path:?}: {e}");
        }
    }
}
