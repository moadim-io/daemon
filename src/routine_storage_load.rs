//! Load/scan the on-disk routines directory into a [`RoutineStore`], and the base-dir-aware
//! read helpers that make a reload coherent for any scan root (production dir or a test tempdir).
//!
//! Split out of `routine_storage.rs` to stay under the repo's 500-line-per-file cap; the
//! `RoutineToml`/`RuntimeState` structs and the write path stay in the parent module.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::paths::routines_dir;
use crate::routines::{Routine, RoutineStore};
use crate::utils::lock::LockRecover;

use super::{read_routine_cron, read_routine_toml, read_runtime_state};

#[path = "routine_storage_walk.rs"]
mod routine_storage_walk;

/// Read the last Unix-timestamp line from an append-only trigger log (e.g. `scheduled.log` or
/// `manual.log`), returning `None` when the file is absent or contains no parsable timestamp.
fn read_last_log_timestamp(path: &std::path::Path) -> Option<u64> {
    let text = std::fs::read_to_string(path).ok()?;
    text.lines()
        .rev()
        .find_map(|line| line.trim().parse::<u64>().ok())
}

/// Read `last_scheduled_trigger_at` from a routine's `scheduled.log` under `base`, returning
/// `None` when the log is absent or empty (e.g. before the routine's schedule has ever fired).
fn read_scheduled_state(base: &std::path::Path, dir_name: &str) -> Option<u64> {
    read_last_log_timestamp(&base.join(dir_name).join("scheduled.log"))
}

/// Read `last_manual_trigger_at` from a routine's `manual.log` under `base`, returning `None`
/// when the log is absent or empty.
fn read_manual_state(base: &std::path::Path, dir_name: &str) -> Option<u64> {
    read_last_log_timestamp(&base.join(dir_name).join("manual.log"))
}

/// Read a routine's raw prompt from its `prompts/prompt.pure.md` sidecar under `base`, falling
/// back to the legacy `routine.toml` `prompt` field for a dir that has not been migrated yet.
fn read_pure_prompt(base: &std::path::Path, dir_name: &str, legacy: Option<String>) -> String {
    std::fs::read_to_string(base.join(dir_name).join("prompts").join("prompt.pure.md"))
        .ok()
        .or(legacy)
        .unwrap_or_default()
}

/// Load a routine from `{routines_dir}/{dir_name}/routine.toml` (the production base directory).
///
/// `dir_name` is the slug (title-derived folder name). The routine's UUID `id` is read from
/// `routine.toml`; for legacy dirs created before this change `id` falls back to `dir_name`.
///
/// `snoozed_until` and `skip_runs` are read from the `state.local.toml` sidecar;
/// `last_manual_trigger_at`/`last_scheduled_trigger_at` are read from the `manual.log`/
/// `scheduled.log` append-only logs, falling back to legacy fields for routines written before
/// those were split out.
pub(super) fn load_routine_from_dir(dir_name: &str) -> Option<Routine> {
    load_routine_from_base(&routines_dir(), dir_name)
}

/// Load a routine from `{base}/{dir_name}/routine.toml`, reading every sidecar (`schedule.cron`,
/// `state.local.toml`, `manual.log`, `scheduled.log`, `prompts/prompt.pure.md`) from the same
/// `{base}`.
///
/// This is the directory-coherent variant of [`load_routine_from_dir`]: every file is resolved
/// relative to `base` rather than the global [`routines_dir`], so a reload against a tempdir (tests)
/// or the production directory both read a self-consistent set of files. The scheduler-written
/// `last_scheduled_trigger_at` is read back here, so a reload through this path preserves it.
fn load_routine_from_base(base: &std::path::Path, dir_name: &str) -> Option<Routine> {
    let toml = read_routine_toml(&base.join(dir_name).join("routine.toml"))?;
    let title = toml.title?;
    let id = toml.id.unwrap_or_else(|| dir_name.to_string());
    let runtime_state = read_runtime_state(base, dir_name);
    // routine.toml is authoritative (schedule.cron only mirrors it and is not functional yet);
    // fall back to the cron sidecar so dirs written while the schedule lived only there keep
    // loading until repersisted.
    let schedule = toml
        .schedule
        .or_else(|| read_routine_cron(&base.join(dir_name).join("schedule.cron")))?;
    // Prefer the log file; fall back to legacy state.local.toml field then routine.toml field for
    // routines that predate the log-file migration.
    let last_manual_trigger_at = read_manual_state(base, dir_name)
        .or(runtime_state.last_manual_trigger_at)
        .or(toml.last_manual_trigger_at);
    let last_scheduled_trigger_at = read_scheduled_state(base, dir_name);
    let prompt = read_pure_prompt(base, dir_name, toml.prompt);
    Some(Routine {
        id,
        schedule,
        title,
        agent: toml.agent?,
        model: toml.model,
        prompt,
        goal: toml.goal,
        repositories: toml.repositories,
        machines: toml.machines,
        enabled: toml.enabled.unwrap_or(true),
        source: "managed".to_string(),
        created_at: toml.created_at.unwrap_or(0),
        updated_at: toml.updated_at.unwrap_or(0),
        last_manual_trigger_at,
        last_scheduled_trigger_at,
        snoozed_until: runtime_state.snoozed_until,
        skip_runs: runtime_state.skip_runs,
        power_saving: runtime_state.power_saving,
        ttl_secs: toml.ttl_secs,
        max_runtime_secs: toml.max_runtime_secs,
        tags: toml.tags,
        env: toml.env,
    })
}

/// Scan `~/.config/moadim/routines/` and load all valid routines into a new store.
pub fn load_store() -> RoutineStore {
    load_store_from_dir(&routines_dir())
}

/// Scan `dir` and load all valid routines into a new store.
///
/// Every per-routine file (the tracked `routine.toml`, the tracked `schedule.cron`, the
/// `state.local.toml` sidecar, the `manual.log`/`scheduled.log` trigger logs, and the
/// `prompts/prompt.pure.md` sidecar) is read relative to `dir`, so the scan is fully coherent for
/// any directory — not only the global [`routines_dir`].
pub(crate) fn load_store_from_dir(dir: &std::path::Path) -> RoutineStore {
    Arc::new(Mutex::new(scan_routines(dir)))
}

/// Re-scan `dir` and replace `store`'s contents with the freshly-loaded routines, in place.
///
/// Disk is the source of truth: this lets a routine pulled or edited on disk under a running daemon
/// (e.g. after a `git pull` of the config repo, including a changed `machines` list) become visible
/// on the next read without a restart. Because the reload goes through [`load_routine_from_base`],
/// each routine's gitignored `scheduled.log` (`last_scheduled_trigger_at`) is read back and
/// preserved rather than clobbered. Routines whose dir disappeared on disk drop out of the store.
///
/// The whole map is replaced under a single lock so a concurrent reader never observes a partial
/// store. Every create/update/delete/trigger persists to disk before returning, so replacing the map
/// from disk loses no state.
pub(crate) fn reload_store_from_dir(store: &RoutineStore, dir: &std::path::Path) {
    let fresh = scan_routines(dir);
    *store.lock_recover() = fresh;
}

/// Scan `dir` for routine directories (including nested ones) and return the loaded routines
/// keyed by id.
///
/// Shared by [`load_store_from_dir`] (build a new store) and [`reload_store_from_dir`] (refresh an
/// existing one).
fn scan_routines(dir: &std::path::Path) -> HashMap<String, Routine> {
    let mut routines = HashMap::new();
    routine_storage_walk::walk_routines(dir, dir, &mut routines, &load_routine_from_base);
    routines
}
