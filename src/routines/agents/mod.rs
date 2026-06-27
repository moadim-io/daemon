//! The agent registry: resolving `~/.config/moadim/agents/<name>.toml` and seeding built-in defaults.
//!
//! Each supported agent contributes its registry key and default config from its own
//! `<agent>/setup.rs` module; [`DEFAULT_AGENT_CONFIGS`] assembles them for [`ensure_default_agents`].

use serde::Deserialize;
use std::io::ErrorKind;
use std::path::Path;

use crate::paths::{agent_toml_path, agents_dir};

#[path = "claude_code/setup.rs"]
mod claude_code;
#[path = "codex/setup.rs"]
mod codex;
#[path = "hermes/setup.rs"]
mod hermes;

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
    /// Filename the agent reads its project instructions from, written into the workbench by the
    /// daemon so the moadim system prompt and routine-origin disclosure reach the agent that
    /// actually runs. Claude Code reads `CLAUDE.md` (the default); Codex reads `AGENTS.md`.
    #[serde(default = "default_instructions_file")]
    pub instructions_file: String,
}

/// Default project-instructions filename for an agent: Claude Code's `CLAUDE.md` convention.
///
/// Applied when an agent's TOML omits `instructions_file`, preserving the prior behavior of
/// always writing `CLAUDE.md` for configs that predate this field.
fn default_instructions_file() -> String {
    "CLAUDE.md".to_string()
}

/// Why [`load_agent_command`] could not produce an [`AgentCommand`].
///
/// Distinguishes a genuinely missing config (the routine simply has no `<name>.toml`) from a config
/// that is present on disk but cannot be read or parsed, so callers can report the real cause instead
/// of collapsing every failure into a misleading "config not found".
#[derive(Debug)]
pub enum AgentLoadError {
    /// No `~/.config/moadim/agents/<name>.toml` exists.
    Missing,
    /// The file exists but could not be read (e.g. a permissions error, or the path is a directory);
    /// carries the underlying I/O error. Distinct from [`Missing`](Self::Missing) so an unreadable
    /// config is never mislabeled "not found" and silently dropped.
    Unreadable(String),
    /// The file exists but its TOML could not be parsed; carries the underlying parse error.
    Parse(String),
}

impl std::fmt::Display for AgentLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AgentLoadError::Missing => write!(f, "agent config not found"),
            AgentLoadError::Unreadable(err) => write!(f, "unreadable agent config: {err}"),
            AgentLoadError::Parse(err) => write!(f, "malformed agent TOML: {err}"),
        }
    }
}

/// Load the agent command for `name`.
///
/// Returns [`AgentLoadError::Missing`] only when no config file exists, [`AgentLoadError::Unreadable`]
/// when the file is present but cannot be read (e.g. a permissions error), and
/// [`AgentLoadError::Parse`] (carrying the `toml` error) when the file is present but unparseable, so
/// the three failures are never conflated.
pub fn load_agent_command(name: &str) -> Result<AgentCommand, AgentLoadError> {
    let text = std::fs::read_to_string(agent_toml_path(name)).map_err(|err| {
        if err.kind() == ErrorKind::NotFound {
            AgentLoadError::Missing
        } else {
            AgentLoadError::Unreadable(err.to_string())
        }
    })?;
    toml::from_str(&text).map_err(|err| AgentLoadError::Parse(err.to_string()))
}

/// Built-in default agent configs `(name, toml)`, written on startup if the file does not exist.
const DEFAULT_AGENT_CONFIGS: &[(&str, &str)] = &[
    (claude_code::NAME, claude_code::CONFIG),
    (codex::NAME, codex::CONFIG),
    (hermes::NAME, hermes::CONFIG),
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
        .filter_map(Result::ok)
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
