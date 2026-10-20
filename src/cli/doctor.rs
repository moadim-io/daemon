//! `moadim doctor`: local, one-shot host diagnostics. It checks the binaries a routine run needs on
//! `PATH`, that each agent config parses and its `command` resolves, and whether a daemon answers.
//! It runs without a daemon; the daemon check only reports the state.

use std::path::{Path, PathBuf};

use super::is_running;
use crate::paths::agents_dir;
use crate::routines::{agent_command_available_in, AgentCommand};

/// One diagnostic result: a short `label` and, when the check failed, the reason it failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Check {
    /// What was checked, e.g. `tmux` or `agent claude`.
    pub label: String,
    /// Why the check failed; `None` when it passed.
    pub failure: Option<String>,
}

impl Check {
    /// A check that failed for `reason`.
    fn fail(label: impl Into<String>, reason: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            failure: Some(reason.into()),
        }
    }

    /// A pass when `found` holds, otherwise a failure carrying `reason`.
    fn require(label: impl Into<String>, found: bool, reason: &str) -> Self {
        let label = label.into();
        if found {
            Self {
                label,
                failure: None,
            }
        } else {
            Self::fail(label, reason)
        }
    }

    /// Whether this check passed.
    pub const fn passed(&self) -> bool {
        self.failure.is_none()
    }

    /// The one output line for this check: `ok   <label>` or `FAIL <label> — <reason>`.
    pub fn render(&self) -> String {
        match &self.failure {
            None => format!("ok   {}", self.label),
            Some(reason) => format!("FAIL {} — {reason}", self.label),
        }
    }
}

/// Run every check against the given inputs, in print order: `tmux`, `python3`, one `agent <name>`
/// per `*.toml` config in `agents_dir` (sorted by file name), then `daemon`.
///
/// `path` is the `:`-separated `PATH` to search, and `daemon_up` says whether the server answered
/// its health probe. Pure, so tests can pass a fake `PATH` and a temp agents dir.
pub fn run_checks(path: &str, agents_dir: &Path, daemon_up: bool) -> Vec<Check> {
    let mut checks = vec![
        Check::require(
            "tmux",
            agent_command_available_in(path, "tmux"),
            "not found on PATH (routine runs cannot launch)",
        ),
        Check::require(
            "python3",
            agent_command_available_in(path, "python3"),
            "not found on PATH (agent setup steps cannot run)",
        ),
    ];
    checks.extend(agent_checks(path, agents_dir));
    checks.push(Check::require(
        "daemon",
        daemon_up,
        "not running; start it with: moadim",
    ));
    checks
}

/// One [`Check`] per `*.toml` file in `dir`, sorted by path. A missing or unreadable dir yields none.
fn agent_checks(path: &str, dir: &Path) -> Vec<Check> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut files: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|file| file.extension().is_some_and(|ext| ext == "toml"))
        .collect();
    files.sort();
    files.iter().map(|file| agent_check(path, file)).collect()
}

/// Parse one agent config the same way [`crate::routines::load_agent_command`] does, then check
/// its `command` resolves on `path`. A config that does not parse fails with its first error line.
fn agent_check(path: &str, file: &Path) -> Check {
    let name = file.file_stem().and_then(|stem| stem.to_str());
    let label = format!("agent {}", name.unwrap_or_default());
    let loaded = std::fs::read_to_string(file)
        .map_err(|err| err.to_string())
        .and_then(|text| toml::from_str::<AgentCommand>(&text).map_err(|err| err.to_string()));
    match loaded {
        Ok(agent) => Check::require(
            label,
            agent_command_available_in(path, &agent.command),
            &format!("`{}` not found on PATH", agent.command),
        ),
        Err(err) => {
            let first_line = err.lines().next().unwrap_or_default();
            Check::fail(label, first_line)
        }
    }
}

/// Process exit code for `checks`: `0` when every check passed, `1` when any failed.
pub fn exit_code(checks: &[Check]) -> i32 {
    if checks.iter().all(Check::passed) {
        0
    } else {
        1
    }
}

/// Run `moadim doctor` against the live `PATH`, agents dir, and daemon. Prints one line per check
/// and returns the process exit code.
pub fn run() -> i32 {
    let path = std::env::var("PATH").unwrap_or_default();
    let checks = run_checks(&path, &agents_dir(), is_running());
    for check in &checks {
        println!("{}", check.render());
    }
    exit_code(&checks)
}

#[cfg(test)]
#[path = "doctor_tests.rs"]
mod doctor_tests;
