//! Machine identity for multi-machine deployments.
//!
//! One `~/.config/moadim` config repo can be shared (via the user's own git workflow) across several
//! machines — a laptop, a work box, a server. Each routine declares which machines run it through a
//! `machines` targeting list; each daemon then filters its crontab sync to only the entries naming
//! *this* machine. This module answers "which machine am I?".
//!
//! Identity resolves in priority order:
//! 1. the `MOADIM_MACHINE` environment variable (trimmed, non-empty),
//! 2. the `name` field in the gitignored `~/.config/moadim/machine.local.toml`,
//! 3. the system hostname.
//!
//! The file and env override exist because hostnames are not always meaningful or stable; the file
//! is `*.local.*` (gitignored) so a name set on one host never travels in the shared repo.

use std::sync::{Mutex, OnceLock};

use serde::{Deserialize, Serialize};

use crate::paths::machine_config_path;
use crate::utils::atomic::atomic_write;
use crate::utils::lock::LockRecover;

#[allow(
    clippy::missing_docs_in_private_items,
    reason = "split-out module keeps the file under the linecheck limit"
)]
mod glob_match;
pub(crate) use glob_match::*;

/// On-disk shape of `machine.local.toml`.
#[derive(Debug, Default, Deserialize, Serialize)]
struct MachineToml {
    /// This machine's identity name, matched against routine/job `machines` lists.
    name: Option<String>,
    /// UI/REST-configured override for the global routine concurrency cap (issue #1155).
    /// `MAX_CONCURRENT_RUNS_ENV` takes precedence over this when set; `None` means no override.
    max_concurrent_runs: Option<usize>,
}

/// Where a resolved machine identity came from, for `moadim machine show` to report.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MachineSource {
    /// From the `MOADIM_MACHINE` environment variable.
    Env,
    /// From the `name` field in `machine.local.toml`.
    File,
    /// Auto-generated on first run and written to `machine.local.toml`.
    Generated,
    /// Fell back to the system hostname (only when writing the generated name fails).
    Hostname,
}

impl MachineSource {
    /// Short human label used in CLI output.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Env => "MOADIM_MACHINE env",
            Self::File => "machine.local.toml",
            Self::Generated => "auto-generated (first run)",
            Self::Hostname => "system hostname",
        }
    }
}

/// This machine's identity name (just the name, dropping the source).
pub fn current_machine() -> String {
    resolve().0
}

/// This machine's identity name together with where it was resolved from.
pub fn resolve() -> (String, MachineSource) {
    let env = std::env::var("MOADIM_MACHINE").ok();
    let file = read_machine_file();
    if let Some(name) = non_empty(env) {
        return (name, MachineSource::Env);
    }
    if let Some(name) = non_empty(file) {
        return (name, MachineSource::File);
    }
    // No name configured: generate a unique name and persist it so every subsequent
    // call returns the same identity without re-generating.
    let generated = generate_name();
    match set_machine(&generated) {
        Ok(()) => {
            log::warn!(
                "no machine name configured; generated {generated:?} — run `moadim machine set <name>` to choose your own"
            );
            (generated, MachineSource::Generated)
        }
        Err(err) => {
            log::warn!("failed to save generated machine name: {err}; falling back to hostname");
            (hostname(), MachineSource::Hostname)
        }
    }
}

/// Generate a unique machine name of the form `machine-{8hex}`.
fn generate_name() -> String {
    format!(
        "machine-{}",
        &uuid::Uuid::new_v4().simple().to_string()[..8]
    )
}

/// Pure resolution core: pick the first non-empty of env, then file, then hostname.
///
/// Split out from [`resolve`] so the precedence (and each branch) is unit-testable without touching
/// the real environment or filesystem.
#[cfg(test)]
fn resolve_from(
    env: Option<String>,
    file: Option<String>,
    hostname: String,
) -> (String, MachineSource) {
    if let Some(name) = non_empty(env) {
        return (name, MachineSource::Env);
    }
    if let Some(name) = non_empty(file) {
        return (name, MachineSource::File);
    }
    (hostname, MachineSource::Hostname)
}

/// Trim `value` and return it only if it still holds non-whitespace content.
fn non_empty(value: Option<String>) -> Option<String> {
    value
        .map(|raw| raw.trim().to_string())
        .filter(|trimmed| !trimmed.is_empty())
}

/// The system hostname as a lossy UTF-8 string.
fn hostname() -> String {
    gethostname::gethostname().to_string_lossy().into_owned()
}

/// Read the full parsed contents of `machine.local.toml`, or the all-`None` default when the file
/// is absent or unparsable.
fn read_machine_toml() -> MachineToml {
    std::fs::read_to_string(machine_config_path())
        .ok()
        .and_then(|text| toml::from_str(&text).ok())
        .unwrap_or_default()
}

/// Read the `name` field from `machine.local.toml`, or `None` when the file is absent, unparsable,
/// or has no `name` set.
fn read_machine_file() -> Option<String> {
    read_machine_toml().name
}

/// Process-wide lock serializing the `machine.local.toml` read-modify-write sequence.
///
/// [`set_machine`] and [`set_max_concurrent_runs_override`] each read the whole file, mutate one
/// field, and write the whole struct back — an unsynchronized `PUT /machine` and
/// `PUT /config/max-concurrent-runs` (`src/routes/http_settings_routes.rs`) can run concurrently on
/// the multi-thread runtime, so two overlapping round trips can interleave and the later write wins
/// outright, silently dropping whichever field the other request had just persisted. Same hazard
/// class as the crontab read-modify-write race (issue #365, see `crontab_sync_lock` in
/// `src/sync/routines.rs`), fixed the same way: hold this lock across the whole read-then-write span.
fn machine_toml_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}
include!("write_machine_toml.rs");
