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
/// macOS `LaunchAgents` plist) through the same override seam, keeping tests off the real home.
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
    // When the test seam is active, bypass XDG so the entire config tree redirects to the
    // override directory — matching the behaviour callers expect from MOADIM_HOME_OVERRIDE.
    if std::env::var_os(HOME_OVERRIDE_ENV).is_some() {
        config_root_from(None, home())
    } else {
        config_root_from(std::env::var_os(XDG_CONFIG_HOME_ENV), home())
    }
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
#[must_use]
pub fn config_dir() -> PathBuf {
    config_root().join("moadim")
}

// ─── Routines ────────────────────────────────────────────────────────────────

/// Returns the path to `{config_dir}/routines/` (default `~/.config/moadim/routines/`).
#[must_use]
pub fn routines_dir() -> PathBuf {
    config_dir().join("routines")
}

/// Returns the path to `{routines_dir}/{id}/`.
#[must_use]
pub fn routine_dir(id: &str) -> PathBuf {
    routines_dir().join(id)
}

/// Returns the path to `{routines_dir}/README.md`, a daemon-generated orientation doc explaining
/// the per-routine directory layout.
#[must_use]
pub fn routines_readme_path() -> PathBuf {
    routines_dir().join("README.md")
}

/// Returns the path to `{routines_dir}/{id}/routine.toml`, the tracked routine metadata.
#[must_use]
pub fn routine_toml_path(id: &str) -> PathBuf {
    routine_dir(id).join("routine.toml")
}

/// Returns the path to `{routines_dir}/{id}/schedule.cron`, the routine's tracked cron entry.
#[must_use]
pub fn routine_cron_path(id: &str) -> PathBuf {
    routine_dir(id).join("schedule.cron")
}

/// Returns the path to `{routines_dir}/{id}/prompts/`.
#[must_use]
pub fn routine_prompts_dir(id: &str) -> PathBuf {
    routine_dir(id).join("prompts")
}

/// Returns the path to `{routines_dir}/{id}/prompts/prompt.pure.md`, the raw user-authored prompt.
#[must_use]
pub fn routine_pure_prompt_path(id: &str) -> PathBuf {
    routine_prompts_dir(id).join("prompt.pure.md")
}

/// Returns the path to `{routines_dir}/{id}/prompts/prompt.compiled.local.md`, the composed prompt
/// (repositories preamble + pure prompt) that the launch command copies into the workbench.
///
/// `.local.` keeps it matching the routine `.gitignore`'s `*.local.*` pattern: it is fully derived
/// from `prompt.pure.md` + `routine.toml` and rewritten on every [`crate::routine_storage::write_routine`]
/// call, so (unlike `prompt.pure.md`) it should never be tracked (issue #1046).
#[must_use]
pub fn routine_compiled_prompt_path(id: &str) -> PathBuf {
    routine_prompts_dir(id).join("prompt.compiled.local.md")
}

/// Returns the path to `{routines_dir}/{id}/.gitignore`.
#[must_use]
pub fn routine_gitignore_path(id: &str) -> PathBuf {
    routine_dir(id).join(".gitignore")
}

/// Returns the path to `{routines_dir}/{id}/state.local.toml`, the gitignored sidecar holding
/// daemon-written runtime state (`snoozed_until`, `skip_runs`) kept out of the tracked `routine.toml`.
///
/// The `.local.` infix matches the `*.local.*` pattern seeded into each routine's `.gitignore`, so
/// snooze churn never produces version-control diffs.
#[must_use]
pub fn routine_state_path(id: &str) -> PathBuf {
    routine_dir(id).join("state.local.toml")
}

/// Returns the path to `{routines_dir}/{id}/scheduled.log`, the gitignored append-only log that
/// records every scheduled (cron) firing as one Unix-timestamp line.
///
/// The cron shell command appends a line (`printf '%s\n' "$TS" >> scheduled.log`) at each firing;
/// the daemon reads only the last line to derive `last_scheduled_trigger_at`. The `.log` suffix
/// matches the `*.log` pattern seeded into each routine's `.gitignore`.
#[must_use]
pub fn routine_scheduled_log_path(id: &str) -> PathBuf {
    routine_dir(id).join("scheduled.log")
}

/// Returns the path to `{routines_dir}/{id}/manual.log`, the gitignored append-only log that
/// records every manual trigger as one Unix-timestamp line.
///
/// The daemon appends a line at each manual trigger; reading the last line gives
/// `last_manual_trigger_at`. The `.log` suffix matches the `*.log` pattern in the routine's
/// `.gitignore`.
#[must_use]
pub fn routine_manual_log_path(id: &str) -> PathBuf {
    routine_dir(id).join("manual.log")
}

/// Returns the path to `{routines_dir}/{id}/skip.log`, the gitignored append-only log recording
/// why a trigger did not spawn a workbench (agent load failure, an oversized inline prompt, the
/// per-routine overlap guard, or the global concurrency cap — see
/// `crate::routines::service_trigger::spawn_routine_command`).
///
/// Without this, a skipped trigger left no trace anywhere a caller could read back: `routine_logs`
/// looks up the newest *workbench's* `agent.log`, and a skipped trigger never creates a workbench
/// (#1145). The `.log` suffix matches the `*.log` pattern in the routine's `.gitignore`.
#[must_use]
pub fn routine_skip_log_path(id: &str) -> PathBuf {
    routine_dir(id).join("skip.log")
}

/// Returns the path to `{routines_dir}/{id}/runs.log`, the gitignored append-only NDJSON log of
/// every finished run's outcome, keyed by the routine's stable UUID (unlike its workbenches, which
/// are keyed by slug and reaped after their TTL).
///
/// One compact JSON object is appended per run, right before its workbench is reaped (see
/// `routines::cleanup::reap_dir`), so run history survives past workbench retention instead of
/// disappearing the moment its workbench directory is removed. The `.log` suffix matches the
/// `*.log` pattern seeded into each routine's `.gitignore`.
#[must_use]
pub fn routine_run_history_path(id: &str) -> PathBuf {
    routine_dir(id).join("runs.log")
}

/// Returns the path to `{config_dir}/removed_defaults.local.toml`, the gitignored file recording
/// which built-in default routines the user has explicitly deleted, so
/// [`crate::routines::ensure_default_routines`] does not resurrect them on the next startup. The
/// `.local.` infix matches the `*.local.*` pattern seeded into the config `.gitignore`.
#[must_use]
pub fn removed_default_routines_path() -> PathBuf {
    config_dir().join("removed_defaults.local.toml")
}

/// Returns the path to `{routines_dir}/{id}/run.sh`, a legacy per-routine launch script.
///
/// No longer generated — the crontab line now invokes `moadim schedule trigger <id>` directly. The
/// path is retained so [`crate::routine_storage::write_routine`] can delete any stale script left by
/// an older daemon.
#[must_use]
pub fn routine_script_path(id: &str) -> PathBuf {
    routine_dir(id).join("run.sh")
}

/// Returns the path to `{routines_dir}/{id}/flags/`, holding one file per open flag an agent (or a
/// human, via MCP/HTTP) has raised against the routine — a gap, bug, edge case, or question it
/// couldn't resolve mid-run. See [`crate::routines::flags`].
#[must_use]
pub fn routine_flags_dir(id: &str) -> PathBuf {
    routine_dir(id).join("flags")
}

// ─── Agent registry ──────────────────────────────────────────────────────────

/// Returns the path to `{config_dir}/agents/` (default `~/.config/moadim/agents/`).
#[must_use]
pub fn agents_dir() -> PathBuf {
    config_dir().join("agents")
}

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

#[cfg(test)]
mod mod_tests;
