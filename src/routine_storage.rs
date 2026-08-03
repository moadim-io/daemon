//! TOML-backed persistence for routines, plus the tracked `schedule.cron` and the
//! `prompts/prompt.pure.md` (raw) / `prompts/prompt.compiled.local.md` (composed) sidecar files.

use crate::utils::lock::LockRecover;

#[allow(
    clippy::missing_docs_in_private_items,
    reason = "split-out module keeps the file under the linecheck limit"
)]
#[path = "read_runtime_state.rs"]
mod read_runtime_state;
pub(crate) use read_runtime_state::*;
#[path = "routine_disabled_state.rs"]
mod routine_disabled_state;
pub(crate) use routine_disabled_state::*;

// Re-exported (as `super::routines_dir`) for `routine_storage_migrations`; not called directly
// in this file since `load_store`/`load_store_from_dir` moved to `routine_storage_load`.
use crate::paths::routines_dir;
use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::paths::{
    routine_compiled_prompt_path, routine_cron_path, routine_dir, routine_manual_log_path,
    routine_prompts_dir, routine_pure_prompt_path, routine_script_path, routine_skip_log_path,
    routine_state_path, routine_toml_path,
};
use crate::routines::{
    compose_prompt, slugify, FailureNotificationConfig, Repository, Routine, RoutineStore,
};
use crate::utils::atomic::atomic_write;

/// TOML representation of a routine on disk.
#[derive(Debug, Deserialize, Serialize)]
struct RoutineToml {
    /// UUID that uniquely identifies this routine (stable across renames).
    id: Option<String>,

    /// Human name.
    title: Option<String>,
    /// Agent registry key.
    agent: Option<String>,
    /// Model ID override for the agent invocation; absent means the agent's own default.
    #[serde(default)]
    model: Option<String>,
    /// Task prompt.
    ///
    /// **Read-only / legacy.** The prompt now lives in the `prompts/prompt.pure.md` sidecar so it
    /// is diff/edit-friendly markdown instead of an escaped TOML string. This field is still parsed
    /// so routines written by older daemons keep their prompt (the value migrates into the sidecar
    /// via [`migrate_prompts_to_subfolder`] on the next startup), but it is never written back —
    /// `skip_serializing` keeps it out of every freshly written `routine.toml`.
    #[serde(default, skip_serializing)]
    prompt: Option<String>,
    /// Short (≤5 line) goal statement; absent means unset.
    #[serde(default)]
    goal: Option<String>,
    /// Context repositories.
    #[serde(default)]
    repositories: Vec<Repository>,
    /// Machines this routine is assigned to run on (empty = nowhere). Tracked config: the
    /// targeting decision is authored once in the shared repo, not per-machine sidecar state.
    #[serde(default)]
    machines: Vec<String>,
    /// Whether the routine is enabled.
    ///
    /// **Read-only / legacy.** Disable intent now lives in the tracked `disabled.json` marker:
    /// marker present means disabled, marker absent means enabled. This field is still parsed so
    /// older routine.toml files with `enabled = false` continue to load during migration, but it is
    /// never written back.
    #[serde(default, skip_serializing)]
    enabled: Option<bool>,
    /// Whether the routine may launch while the host is in system power saving.
    #[serde(default)]
    power_saving_exempt: bool,
    /// Unix creation timestamp.
    created_at: Option<u64>,
    /// Unix last-updated timestamp.
    updated_at: Option<u64>,
    /// Unix timestamp of last manual trigger.
    ///
    /// **Read-only / legacy.** Runtime trigger state now lives in the gitignored `state.local.toml`
    /// sidecar ([`RuntimeState`]) so it no longer churns the version-controlled `routine.toml`.
    /// This field is still parsed so routines written by older daemons keep their timestamp (the
    /// value migrates into the sidecar on the next [`write_routine`]), but it is never written back
    /// — `skip_serializing` keeps it out of every freshly written `routine.toml`. Accepts the
    /// legacy `last_triggered_at` key so routine.toml files written before the rename still load.
    #[serde(default, skip_serializing, alias = "last_triggered_at")]
    last_manual_trigger_at: Option<u64>,
    /// Workbench retention in seconds for finished runs; absent means the daemon default.
    #[serde(default)]
    ttl_secs: Option<u64>,
    /// Max wall-clock seconds a single run may execute before the watchdog kills it; absent means
    /// the daemon default.
    #[serde(default)]
    max_runtime_secs: Option<u64>,
    /// Consecutive failed-or-unknown runs after which the daemon auto-disables this routine
    /// (the failure circuit-breaker); `None`/`0` opts out. See
    /// [`crate::routines::Routine::failure_threshold`] (#521).
    #[serde(default)]
    failure_threshold: Option<u32>,
    /// Failure notification hook config.
    #[serde(default, skip_serializing_if = "FailureNotificationConfig::is_empty")]
    notifications: FailureNotificationConfig,
    /// Free-form labels for the routine; absent means no tags.
    #[serde(default)]
    tags: Vec<String>,
    /// Non-secret environment variables injected at launch (see [`crate::routines::Routine::env`]).
    /// Absent/empty means none. Tracked and git-committed — never put a secret here; use the
    /// gitignored `routine.local.toml` sidecar instead ([`RoutineLocalToml`]).
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    env: HashMap<String, String>,
}

/// TOML representation of a routine's untracked `routine.local.toml` sidecar: secret or
/// machine-local environment variable overrides that win over `routine.toml`'s `[env]` table at
/// launch time (issue #408).
///
/// Deliberately **not** part of [`RoutineToml`] or [`crate::routines::Routine`] — those are held
/// in the in-memory [`crate::routines::RoutineStore`] and serialized straight into API responses,
/// so a secret parsed into either would leak into every `GET /routines` response the moment it
/// loaded. This struct is read fresh from disk only where a value is actually needed
/// ([`read_local_env`], called from `build_routine_command` at launch time) and discarded
/// immediately after.
#[derive(Debug, Default, Deserialize)]
struct RoutineLocalToml {
    /// Secret / machine-local env var overrides; absent means none.
    #[serde(default)]
    env: HashMap<String, String>,
}

/// Daemon-written runtime state for a routine, persisted to the gitignored `state.local.toml`
/// sidecar so it never appears in the version-controlled `routine.toml`.
///
/// Trigger history (`last_manual_trigger_at`, `last_scheduled_trigger_at`) is no longer stored
/// here — it lives in the append-only `manual.log` / `scheduled.log` files instead.
#[derive(Debug, Default, Deserialize, Serialize)]
pub(crate) struct RuntimeState {
    /// Unix timestamp of the last manual trigger, or `None` if it has never been triggered.
    ///
    /// **Read-only / legacy.** Manual trigger history now lives in the `manual.log` append-only
    /// sidecar. This field is still parsed so routines written by older daemons keep their
    /// timestamp (the value migrates into `manual.log` on the next startup), but it is never
    /// written back — `skip_serializing` keeps it out of every freshly written `state.local.toml`.
    #[serde(default, skip_serializing)]
    last_manual_trigger_at: Option<u64>,
    /// Unix timestamp until which scheduled fires are skipped, or `None`. See
    /// [`crate::routines::Routine::snoozed_until`].
    #[serde(default)]
    snoozed_until: Option<u64>,
    /// Count of upcoming scheduled fires still to skip, or `None`. See
    /// [`crate::routines::Routine::skip_runs`].
    #[serde(default)]
    skip_runs: Option<u32>,
    /// Whether firing is paused for power saving. See
    /// [`crate::routines::Routine::power_saving`].
    #[serde(default)]
    power_saving: bool,
    /// Count of consecutive failed-or-unknown runs. See
    /// [`crate::routines::Routine::consecutive_failures`].
    #[serde(default)]
    consecutive_failures: u32,
    /// Why the failure circuit-breaker last auto-disabled this routine, or `None`. See
    /// [`crate::routines::Routine::auto_disabled_reason`].
    #[serde(default)]
    auto_disabled_reason: Option<String>,
}

/// Parse a routine TOML file at `path`, returning `None` on any error.
fn read_routine_toml(path: &std::path::PathBuf) -> Option<RoutineToml> {
    let text = std::fs::read_to_string(path).ok()?;
    toml::from_str(&text).ok()
}

/// Read a routine's tracked cron entries from `schedule.cron`, returning every line that is neither
/// empty nor a `#` comment.
pub(crate) fn read_routine_crons(path: &std::path::Path) -> Vec<String> {
    let Ok(text) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(ToString::to_string)
        .collect()
}
include!("routine_storage_2.rs");
