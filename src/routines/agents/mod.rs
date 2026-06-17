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
    /// Optional command run in the workbench *before* the agent launches, inserted verbatim into the
    /// generated launch script. The workbench path and session name are in scope — `$WB`/`$SESS` in
    /// the Unix `/bin/sh` line, `$wb`/`$sess` in the Windows `run.ps1` — e.g. to pre-seed
    /// per-directory editor trust state.
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

/// Registry keys of the built-in agents, in declaration order.
fn builtin_agent_names() -> Vec<String> {
    DEFAULT_AGENT_CONFIGS
        .iter()
        .map(|(n, _)| n.to_string())
        .collect()
}

/// Names of all agents the daemon can launch: the `<name>.toml` stems under
/// `~/.config/moadim/agents/`, sorted alphabetically.
///
/// Falls back to the built-in defaults when the directory is unreadable (e.g. before startup
/// seeding), so the list is never empty.
pub fn available_agents() -> Vec<String> {
    available_agents_in(&agents_dir())
}

/// List agent names from `dir`. See [`available_agents`].
pub(crate) fn available_agents_in(dir: &Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return builtin_agent_names();
    };
    let mut names: Vec<String> = entries
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| {
            let path = entry.path();
            (path.extension()? == "toml")
                .then(|| path.file_stem()?.to_str().map(str::to_string))
                .flatten()
        })
        .collect();
    if names.is_empty() {
        return builtin_agent_names();
    }
    names.sort();
    names
}

/// Write any missing built-in agent configs into `~/.config/moadim/agents/`.
///
/// Existing files are never overwritten, so user edits are preserved. Best-effort: directory or
/// write failures are logged and ignored rather than aborting startup.
pub fn ensure_default_agents() {
    ensure_default_agents_in(&agents_dir());
}

/// Write missing built-in agent configs into `dir`. See [`ensure_default_agents`].
pub(crate) fn ensure_default_agents_in(dir: &Path) {
    if let Err(err) = std::fs::create_dir_all(dir) {
        log::warn!("ensure_default_agents: failed to create {dir:?}: {err}");
        return;
    }
    for (name, contents) in DEFAULT_AGENT_CONFIGS {
        let path = dir.join(format!("{name}.toml"));
        if path.exists() {
            continue;
        }
        if let Err(err) = std::fs::write(&path, contents) {
            log::warn!("ensure_default_agents: failed to write {path:?}: {err}");
        }
    }
}

#[cfg(test)]
#[path = "agents_tests.rs"]
mod agents_tests;
