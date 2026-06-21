//! Path builders for the moadim jobs and handlers directory layout.

use std::path::PathBuf;

/// Environment variable that, when set, overrides the home directory all moadim paths resolve
/// under. Used by tests to redirect config/routines/jobs/agents/workbenches into a tempdir so they
/// never read or write the user's real `~/.config/moadim`.
const HOME_OVERRIDE_ENV: &str = "MOADIM_HOME_OVERRIDE";

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

/// Returns the path to `{routines_dir}/{id}/state.local.toml`, the gitignored sidecar holding
/// daemon-written runtime state (e.g. `last_manual_trigger_at`) kept out of the tracked `routine.toml`.
///
/// The `.local.` infix matches the `*.local.*` pattern seeded into each routine's `.gitignore`, so
/// trigger churn never produces version-control diffs.
pub fn routine_state_path(id: &str) -> PathBuf {
    routine_dir(id).join("state.local.toml")
}

/// Returns the path to `{routines_dir}/{id}/scheduled.local.toml`, the gitignored sidecar that
/// records `last_scheduled_trigger_at`.
///
/// Unlike [`routine_state_path`] this sidecar is written by the routine's launch command (the
/// `printf` step of [`crate::routines::build_routine_command`]) at each scheduled cron firing, and is
/// only ever *read* by the daemon — kept in its own file so a daemon-side re-persist of
/// `state.local.toml` can't clobber the scheduler-written timestamp. The `.local.` infix matches the
/// `*.local.*` `.gitignore` pattern, so scheduled-fire churn never produces version-control diffs.
pub fn routine_scheduled_state_path(id: &str) -> PathBuf {
    routine_dir(id).join("scheduled.local.toml")
}

/// Returns the path to `{routines_dir}/{id}/run.sh`, a legacy per-routine launch script.
///
/// No longer generated — the crontab line now invokes `moadim schedule trigger <id>` directly. The
/// path is retained so [`crate::routine_storage::write_routine`] can delete any stale script left by
/// an older daemon.
pub fn routine_script_path(id: &str) -> PathBuf {
    routine_dir(id).join("run.sh")
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

/// Returns the path to `~/.config/moadim/machine.local.toml`, the gitignored, per-machine file
/// that records this install's machine identity (the `name` used to match a routine/job's
/// `machines` targeting list). The `.local.` infix matches the `*.local.*` pattern seeded into the
/// config `.gitignore`, so a machine name set on one host never leaks into the shared config repo.
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
