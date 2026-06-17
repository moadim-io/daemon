//! Path builders for the moadim jobs and handlers directory layout.

use std::path::PathBuf;

/// Environment variable that, when set, overrides the home directory all moadim paths resolve
/// under. Used by tests to redirect config/routines/jobs/agents/workbenches into a tempdir so they
/// never read or write the user's real `~/.config/moadim`.
const HOME_OVERRIDE_ENV: &str = "MOADIM_HOME_OVERRIDE";

/// Resolve the base home directory, honoring the [`HOME_OVERRIDE_ENV`] test seam when set.
fn home() -> Option<PathBuf> {
    match std::env::var_os(HOME_OVERRIDE_ENV) {
        Some(dir) => Some(PathBuf::from(dir)),
        None => dirs::home_dir(),
    }
}

/// Returns the path to `~/.config/moadim/jobs/`.
pub fn jobs_dir() -> PathBuf {
    jobs_dir_from_home(home())
}

/// Returns the jobs directory under `home`, or `.` if `home` is `None`.
pub(crate) fn jobs_dir_from_home(home: Option<PathBuf>) -> PathBuf {
    home.unwrap_or_else(|| PathBuf::from("."))
        .join(".config")
        .join("moadim")
        .join("jobs")
}

/// Returns the path to `~/.config/moadim/handlers/`.
pub fn handlers_dir() -> PathBuf {
    handlers_dir_from_home(home())
}

/// Returns the handlers directory under `home`, or `.` if `home` is `None`.
pub(crate) fn handlers_dir_from_home(home: Option<PathBuf>) -> PathBuf {
    home.unwrap_or_else(|| PathBuf::from("."))
        .join(".config")
        .join("moadim")
        .join("handlers")
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

/// Returns the path to `~/.config/moadim/routines/`.
pub fn routines_dir() -> PathBuf {
    routines_dir_from_home(home())
}

/// Returns the routines directory under `home`, or `.` if `home` is `None`.
pub(crate) fn routines_dir_from_home(home: Option<PathBuf>) -> PathBuf {
    home.unwrap_or_else(|| PathBuf::from("."))
        .join(".config")
        .join("moadim")
        .join("routines")
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

/// Returns the path to the generated launch script invoked by the OS scheduler.
///
/// `run.sh` on Unix (invoked by cron via `/bin/sh`); `run.ps1` on Windows (invoked by Task
/// Scheduler via `powershell -File`).
#[cfg(not(windows))]
pub fn routine_script_path(id: &str) -> PathBuf {
    routine_dir(id).join("run.sh")
}

/// Returns the path to `{routines_dir}/{id}/run.ps1`, the PowerShell launch script invoked by the
/// Windows Task Scheduler.
#[cfg(windows)]
pub fn routine_script_path(id: &str) -> PathBuf {
    routine_dir(id).join("run.ps1")
}

// ─── Agent registry ──────────────────────────────────────────────────────────

/// Returns the path to `~/.config/moadim/agents/`.
pub fn agents_dir() -> PathBuf {
    agents_dir_from_home(home())
}

/// Returns the agents directory under `home`, or `.` if `home` is `None`.
pub(crate) fn agents_dir_from_home(home: Option<PathBuf>) -> PathBuf {
    home.unwrap_or_else(|| PathBuf::from("."))
        .join(".config")
        .join("moadim")
        .join("agents")
}

/// Returns the path to `~/.config/moadim/agents/{name}.toml`.
pub fn agent_toml_path(name: &str) -> PathBuf {
    agents_dir().join(format!("{name}.toml"))
}

// ─── Daemon runtime files ────────────────────────────────────────────────────

/// Returns the path to `~/.config/moadim/`.
pub fn config_dir() -> PathBuf {
    config_dir_from_home(home())
}

/// Returns the moadim config directory under `home`, or `.` if `home` is `None`.
pub(crate) fn config_dir_from_home(home: Option<PathBuf>) -> PathBuf {
    home.unwrap_or_else(|| PathBuf::from("."))
        .join(".config")
        .join("moadim")
}

/// Returns the path to `~/.config/moadim/moadim.pid`, where the running server records its PID.
pub fn pid_file() -> PathBuf {
    config_dir().join("moadim.pid")
}

/// Returns the path to `~/.config/moadim/daemon.log`, where a backgrounded server writes its output.
pub fn daemon_log_file() -> PathBuf {
    config_dir().join("daemon.log")
}

/// Returns the path to `~/.config/moadim/.gitignore`, used to keep generated runtime
/// files (`*.pid`, `*.log`) out of version control when the config dir is tracked.
pub fn config_gitignore_path() -> PathBuf {
    config_dir().join(".gitignore")
}

// ─── System prompts ──────────────────────────────────────────────────────────

/// Returns the path to `~/.config/moadim/user_prompt.md`, where the user writes a persistent
/// system prompt injected into every agent workbench `CLAUDE.md` alongside the moadim prompt.
pub fn user_prompt_path() -> PathBuf {
    user_prompt_path_from_home(home())
}

/// Returns the user prompt path under `home`, or `.` if `home` is `None`.
pub(crate) fn user_prompt_path_from_home(home: Option<PathBuf>) -> PathBuf {
    home.unwrap_or_else(|| PathBuf::from("."))
        .join(".config")
        .join("moadim")
        .join("user_prompt.md")
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
