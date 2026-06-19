//! Path builders for the moadim jobs and handlers directory layout.

use std::ffi::OsString;
use std::path::PathBuf;

/// Environment variable that, when set, overrides the home directory all moadim paths resolve
/// under. Used by tests to redirect config/routines/jobs/agents/workbenches into a tempdir so they
/// never read or write the user's real `~/.config/moadim`.
const HOME_OVERRIDE_ENV: &str = "MOADIM_HOME_OVERRIDE";

/// Environment variable from the XDG Base Directory spec that relocates the user's config root.
const XDG_CONFIG_HOME_ENV: &str = "XDG_CONFIG_HOME";

/// Resolve the base home directory, honoring the [`HOME_OVERRIDE_ENV`] test seam when set.
///
/// Exposed to the crate so platform service installers resolve their home-relative paths (e.g. the
/// macOS LaunchAgents plist) through the same override seam, keeping tests off the real home.
pub(crate) fn home() -> Option<PathBuf> {
    match std::env::var_os(HOME_OVERRIDE_ENV) {
        Some(dir) => Some(PathBuf::from(dir)),
        None => dirs::home_dir(),
    }
}

/// Resolve the config root the moadim config tree nests under, honoring the XDG Base Directory
/// spec.
///
/// When `$XDG_CONFIG_HOME` is set to an **absolute** path it is used verbatim; an unset, empty, or
/// relative value falls back to `$HOME/.config`. This mirrors the `dirs` crate that the Linux
/// systemd installer ([`crate::service`]) already uses for the unit path, so a user who relocates
/// their config root via `$XDG_CONFIG_HOME` gets a single coherent config tree instead of a
/// surprise second one under `~/.config`.
fn config_root() -> PathBuf {
    config_root_from(std::env::var_os(XDG_CONFIG_HOME_ENV), home())
}

/// Resolve the config root from an explicit `$XDG_CONFIG_HOME` value and home directory.
///
/// Split out from [`config_root`] so the resolution rules are unit-testable without mutating
/// process-global environment variables. A relative `$XDG_CONFIG_HOME` is ignored, per the spec
/// ("All paths set in these environment variables must be absolute"). Falls back to `.` when the
/// home directory is undeterminable.
fn config_root_from(xdg: Option<OsString>, home: Option<PathBuf>) -> PathBuf {
    if let Some(raw) = xdg {
        let candidate = PathBuf::from(raw);
        if candidate.is_absolute() {
            return candidate;
        }
    }
    home.unwrap_or_else(|| PathBuf::from(".")).join(".config")
}

/// Returns the moadim config directory: `$XDG_CONFIG_HOME/moadim`, defaulting to `~/.config/moadim`.
pub fn config_dir() -> PathBuf {
    config_root().join("moadim")
}

/// Returns the path to `{config_dir}/jobs/` (default `~/.config/moadim/jobs/`).
pub fn jobs_dir() -> PathBuf {
    config_dir().join("jobs")
}

/// Returns the path to `{config_dir}/handlers/` (default `~/.config/moadim/handlers/`).
pub fn handlers_dir() -> PathBuf {
    config_dir().join("handlers")
}

/// Returns the path to `{jobs_dir}/{id}/`.
pub fn job_dir(id: &str) -> PathBuf {
    jobs_dir().join(id)
}

/// Returns the path to `{jobs_dir}/{id}/job.toml`.
pub fn job_toml_path(id: &str) -> PathBuf {
    job_dir(id).join("job.toml")
}

/// Returns the path to `{jobs_dir}/{id}/job.local.toml`.
pub fn job_local_toml_path(id: &str) -> PathBuf {
    job_dir(id).join("job.local.toml")
}

/// Returns the path to `{jobs_dir}/{id}/.gitignore`.
pub fn job_gitignore_path(id: &str) -> PathBuf {
    job_dir(id).join(".gitignore")
}

/// Returns the path to `{jobs_dir}/{id}/job.local.log`.
pub fn job_log_path(id: &str) -> PathBuf {
    job_dir(id).join("job.local.log")
}

// ─── Routines ────────────────────────────────────────────────────────────────

/// Returns the path to `{config_dir}/routines/` (default `~/.config/moadim/routines/`).
pub fn routines_dir() -> PathBuf {
    config_dir().join("routines")
}

/// Returns the path to `{routines_dir}/{id}/`.
pub fn routine_dir(id: &str) -> PathBuf {
    routines_dir().join(id)
}

/// Returns the path to `{routines_dir}/{id}/routine.toml`.
pub fn routine_toml_path(id: &str) -> PathBuf {
    routine_dir(id).join("routine.toml")
}

/// Returns the path to `{routines_dir}/{id}/prompt.md`.
pub fn routine_prompt_path(id: &str) -> PathBuf {
    routine_dir(id).join("prompt.md")
}

/// Returns the path to `{routines_dir}/{id}/.gitignore`.
pub fn routine_gitignore_path(id: &str) -> PathBuf {
    routine_dir(id).join(".gitignore")
}

/// Returns the path to `{routines_dir}/{id}/state.local.toml`, the gitignored sidecar holding
/// daemon-written runtime state (e.g. `last_manual_trigger_at`) kept out of the tracked `routine.toml`.
///
/// The `.local.` infix matches the `*.local.*` pattern seeded into each routine's `.gitignore`, so
/// trigger churn never produces version-control diffs.
pub fn routine_state_path(id: &str) -> PathBuf {
    routine_dir(id).join("state.local.toml")
}

/// Returns the path to `{routines_dir}/{id}/run.sh`, the generated launch script invoked by cron.
pub fn routine_script_path(id: &str) -> PathBuf {
    routine_dir(id).join("run.sh")
}

// ─── Agent registry ──────────────────────────────────────────────────────────

/// Returns the path to `{config_dir}/agents/` (default `~/.config/moadim/agents/`).
pub fn agents_dir() -> PathBuf {
    config_dir().join("agents")
}

/// Returns the path to `{agents_dir}/{name}.toml`.
pub fn agent_toml_path(name: &str) -> PathBuf {
    agents_dir().join(format!("{name}.toml"))
}

// ─── Daemon runtime files ────────────────────────────────────────────────────

/// Returns the path to `{config_dir}/moadim.pid`, where the running server records its PID.
pub fn pid_file() -> PathBuf {
    config_dir().join("moadim.pid")
}

/// Returns the path to `{config_dir}/daemon.log`, where a backgrounded server writes its output.
pub fn daemon_log_file() -> PathBuf {
    config_dir().join("daemon.log")
}

/// Returns the path to `{config_dir}/.gitignore`, used to keep generated runtime
/// files (`*.pid`, `*.log`) out of version control when the config dir is tracked.
pub fn config_gitignore_path() -> PathBuf {
    config_dir().join(".gitignore")
}

// ─── System prompts ──────────────────────────────────────────────────────────

/// Returns the path to `{config_dir}/user_prompt.md`, where the user writes a persistent
/// system prompt injected into every agent workbench `CLAUDE.md` alongside the moadim prompt.
pub fn user_prompt_path() -> PathBuf {
    config_dir().join("user_prompt.md")
}

// ─── Workbenches ─────────────────────────────────────────────────────────────

/// Returns the path to `~/.moadim/`.
pub fn moadim_home() -> PathBuf {
    moadim_home_from_home(home())
}

/// Returns the moadim home directory under `home`, or `.` if `home` is `None`.
pub(crate) fn moadim_home_from_home(home: Option<PathBuf>) -> PathBuf {
    home.unwrap_or_else(|| PathBuf::from(".")).join(".moadim")
}

/// Returns the path to `~/.moadim/workbenches/`.
pub fn workbenches_dir() -> PathBuf {
    moadim_home().join("workbenches")
}

#[cfg(test)]
mod mod_tests;
