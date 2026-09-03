//! Path builders for the moadim jobs and handlers directory layout.

use std::ffi::OsString;
use std::path::PathBuf;

#[allow(
    clippy::missing_docs_in_private_items,
    reason = "split-out module keeps the file under the linecheck limit"
)]
mod agent_toml_path;
pub(crate) use agent_toml_path::*;

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

/// Returns the path to `{config_dir}/notifications.toml`, the optional global failure-hook config.
#[must_use]
pub fn notifications_toml_path() -> PathBuf {
    config_dir().join("notifications.toml")
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

/// Returns the path to `{routines_dir}/{id}/disabled.json`, the tracked marker whose presence
/// disables the routine while carrying basic audit metadata.
#[must_use]
pub fn routine_disabled_json_path(id: &str) -> PathBuf {
    routine_dir(id).join("disabled.json")
}

/// Returns the path to `{routines_dir}/{id}/overlap.json`, the tracked policy that controls
/// whether a routine may launch while one of its earlier fires is still running.
#[must_use]
pub fn routine_overlap_json_path(id: &str) -> PathBuf {
    routine_dir(id).join("overlap.json")
}

/// Returns the path to `{routines_dir}/{id}/schedule.compailed.cron`, the gitignored cron-union output.
///
/// `schedule.cron` stays the human-authored source; this derived file is rewritten by crontab
/// sync and ignored by git.
#[must_use]
pub fn routine_compailed_cron_path(id: &str) -> PathBuf {
    routine_dir(id).join("schedule.compailed.cron")
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
/// `.local.` keeps it matching the config `.gitignore`'s `*.local.*` pattern: it is fully derived
/// from `prompt.pure.md` + `routine.toml` and rewritten on every [`crate::routine_storage::write_routine`]
/// call, so (unlike `prompt.pure.md`) it should never be tracked (issue #1046).
#[must_use]
pub fn routine_compiled_prompt_path(id: &str) -> PathBuf {
    routine_prompts_dir(id).join("prompt.compiled.local.md")
}

/// Returns the path to `{routines_dir}/{id}/.gitignore`, the legacy per-routine gitignore an
/// older daemon generated. No longer written (the config dir's root `.gitignore` covers every
/// routine directory recursively); an existing file is left untouched, since it may carry
/// user-added patterns. Test-only: production code no longer touches this path.
#[cfg(test)]
#[must_use]
pub fn routine_gitignore_path(id: &str) -> PathBuf {
    routine_dir(id).join(".gitignore")
}

/// Returns the path to `{routines_dir}/{id}/state.local.toml`, the gitignored sidecar holding
/// daemon-written runtime state (`snoozed_until`, `skip_runs`) kept out of the tracked `routine.toml`.
///
/// The `.local.` infix matches the `*.local.*` pattern seeded into the config `.gitignore`, so
/// snooze churn never produces version-control diffs.
#[must_use]
pub fn routine_state_path(id: &str) -> PathBuf {
    routine_dir(id).join("state.local.toml")
}

/// Returns the path to `{routines_dir}/{id}/routine.local.toml`, the gitignored sidecar a human
/// (not the daemon) edits directly to layer secret or machine-local environment variable
/// overrides on top of `routine.toml`'s tracked `[env]` table — see
/// [`crate::routines::build_routine_command`] and issue #408.
///
/// The `.local.` infix matches the `*.local.*` pattern seeded into the config `.gitignore`, so it
/// is never accidentally committed.
#[must_use]
pub fn routine_local_toml_path(id: &str) -> PathBuf {
    routine_dir(id).join("routine.local.toml")
}
include!("routine_scheduled_log_path.rs");
