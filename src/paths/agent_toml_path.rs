#![allow(
    clippy::wildcard_imports,
    clippy::too_many_arguments,
    clippy::needless_pass_by_value,
    dead_code,
    reason = "split-out module preserves existing code while keeping files under the linecheck limit"
)]
use super::*;

/// Returns the path to `{agents_dir}/{name}.toml`.
#[must_use]
pub fn agent_toml_path(name: &str) -> PathBuf {
    agents_dir().join(format!("{name}.toml"))
}

/// Returns the path to `{agents_dir}/README.md`, a daemon-generated orientation doc explaining the
/// agent registry's file format.
#[must_use]
pub fn agents_readme_path() -> PathBuf {
    agents_dir().join("README.md")
}

// ─── Daemon runtime files ────────────────────────────────────────────────────

/// Returns the path to `{config_dir}/moadim.pid`, where the running server records its PID.
#[must_use]
pub fn pid_file() -> PathBuf {
    config_dir().join("moadim.pid")
}

/// Returns the path to `{config_dir}/daemon.log`, where a backgrounded server writes its output.
#[must_use]
pub fn daemon_log_file() -> PathBuf {
    config_dir().join("daemon.log")
}

/// Returns the path to `{config_dir}/.gitignore`, used to keep generated runtime
/// files (`*.pid`, `*.log`) out of version control when the config dir is tracked.
#[must_use]
pub fn config_gitignore_path() -> PathBuf {
    config_dir().join(".gitignore")
}

/// Returns the path to `{config_dir}/README.md`, a daemon-generated orientation doc explaining the
/// config tree's layout for anyone who opens or git-tracks it directly.
#[must_use]
pub fn config_readme_path() -> PathBuf {
    config_dir().join("README.md")
}

/// Returns the path to `~/.config/moadim/.lock`, a committed global lock that halts all routine
/// scheduling and manual triggers when present. Checked into version control so the lock can be
/// shared across machines via a git push/pull.
#[must_use]
pub fn global_lock_path() -> PathBuf {
    config_dir().join(".lock")
}

/// Returns the path to `~/.config/moadim/.local.lock`, a machine-local global lock that halts all
/// routine scheduling and manual triggers when present. The `.local.` infix matches the `*.local.*`
/// pattern seeded into the config `.gitignore`, so this sentinel never leaks into version control.
#[must_use]
pub fn global_local_lock_path() -> PathBuf {
    config_dir().join(".local.lock")
}

/// Returns the path to `~/.config/moadim/install_prompt.local.marker`, a machine-local sentinel
/// recording that the post-start "install as a system service?" prompt (see
/// [`crate::cli::run_background`]) has already been shown, so it fires at most once regardless of
/// the answer given. The `.local.` infix matches the `*.local.*` pattern seeded into the config
/// `.gitignore`, so this sentinel never leaks into a shared config repo.
#[must_use]
pub fn install_prompt_marker_path() -> PathBuf {
    config_dir().join("install_prompt.local.marker")
}

/// Returns the path to `~/.config/moadim/machine.local.toml`, the gitignored, per-machine file
/// that records this install's machine identity (the `name` used to match a routine/job's
/// `machines` targeting list). The `.local.` infix matches the `*.local.*` pattern seeded into the
/// config `.gitignore`, so a machine name set on one host never leaks into the shared config repo.
#[must_use]
pub fn machine_config_path() -> PathBuf {
    machine_config_path_from_home(home())
}

/// Returns the machine-config path under `home`, or `.` if `home` is `None`.
pub(crate) fn machine_config_path_from_home(home: Option<PathBuf>) -> PathBuf {
    home.unwrap_or_else(|| PathBuf::from("."))
        .join(".config")
        .join("moadim")
        .join("machine.local.toml")
}

// ─── System prompts ──────────────────────────────────────────────────────────

/// Returns the path to `{config_dir}/user_prompt.md`, where the user writes a persistent
/// system prompt injected into every agent workbench `CLAUDE.md` alongside the moadim prompt.
#[must_use]
pub fn user_prompt_path() -> PathBuf {
    config_dir().join("user_prompt.md")
}

// ─── Repository cache ────────────────────────────────────────────────────────

/// Returns the path to `{config_dir}/cache/`, the root of every repository mirror
/// [`repo_cache_dir`] creates. Used by the cleanup sweep (issue #1425) to walk and prune the whole
/// tree without each caller re-deriving `config_dir().join("cache")` by hand.
#[must_use]
pub fn repo_cache_root_dir() -> PathBuf {
    config_dir().join("cache")
}

/// Returns the path to `{config_dir}/cache/<sanitized-url>`, the persistent local mirror clone of
/// a declared repository (issue #466) — shared across every run, of every routine, that references
/// the same `url`, so a repository is fetched from the remote at most once per fresh URL rather
/// than re-cloned in full on every fire.
///
/// `url` is turned into a directory name by replacing every byte outside `[A-Za-z0-9._-]` with
/// `_`, rather than parsed into host/owner/repo segments: this keeps every valid git remote form
/// (`https://…`, `git@host:owner/repo.git`, `ssh://…`, a local path) supported without a URL
/// parser, at the cost of a longer, less pretty directory name than a host/owner/repo tree would
/// give.
#[must_use]
pub fn repo_cache_dir(url: &str) -> PathBuf {
    repo_cache_root_dir().join(sanitize_repo_cache_name(url))
}

/// Sanitize `url` into a single filesystem-safe path segment for [`repo_cache_dir`].
pub(crate) fn sanitize_repo_cache_name(url: &str) -> String {
    let sanitized: String = url
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '-' | '_') {
                ch
            } else {
                '_'
            }
        })
        .collect();
    if sanitized.is_empty() {
        "repo".to_string()
    } else {
        sanitized
    }
}

// ─── Workbenches ─────────────────────────────────────────────────────────────

/// Returns the path to `~/.moadim/`.
#[must_use]
pub fn moadim_home() -> PathBuf {
    moadim_home_from_home(home())
}

/// Returns the moadim home directory under `home`, or `.` if `home` is `None`.
pub(crate) fn moadim_home_from_home(home: Option<PathBuf>) -> PathBuf {
    home.unwrap_or_else(|| PathBuf::from(".")).join(".moadim")
}

/// Returns the path to `~/.moadim/workbenches/`.
#[must_use]
pub fn workbenches_dir() -> PathBuf {
    moadim_home().join("workbenches")
}

// ─── Claude Code shared config ───────────────────────────────────────────────

/// Returns the path to `~/.claude.json`, the Claude Code config file shared with the live `claude`
/// process. The built-in `claude` agent's `setup` step seeds a per-workbench `projects` entry here
/// on every run (see `crate::routines::agents`); `crate::utils::claude_json` prunes that entry
/// once the cleanup sweep (`crate::routines::cleanup`) reaps the workbench, so the file does not
/// grow unbounded.
///
/// `None` when the home directory cannot be resolved.
#[must_use]
pub fn claude_json_path() -> Option<PathBuf> {
    home().map(|dir| dir.join(".claude.json"))
}
