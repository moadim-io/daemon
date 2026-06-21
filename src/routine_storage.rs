//! TOML-backed persistence for routines, plus the composed `prompt.md` sidecar file.

use crate::utils::lock::LockRecover;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};

use crate::paths::{
    routine_dir, routine_gitignore_path, routine_prompt_path, routine_scheduled_state_path,
    routine_script_path, routine_state_path, routine_toml_path, routines_dir,
};
use crate::routines::{compose_prompt, slugify, Repository, Routine, RoutineStore};
use crate::utils::atomic::atomic_write;

/// TOML representation of a routine on disk.
#[derive(Debug, Deserialize, Serialize)]
struct RoutineToml {
    /// UUID that uniquely identifies this routine (stable across renames).
    id: Option<String>,
    /// Cron expression.
    schedule: Option<String>,
    /// Human name.
    title: Option<String>,
    /// Agent registry key.
    agent: Option<String>,
    /// Task prompt.
    prompt: Option<String>,
    /// Context repositories.
    #[serde(default)]
    repositories: Vec<Repository>,
    /// Whether the routine is enabled.
    enabled: Option<bool>,
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
}

/// Daemon-written runtime state for a routine, persisted to the gitignored `state.local.toml`
/// sidecar so it never appears in the version-controlled `routine.toml`.
#[derive(Debug, Deserialize, Serialize)]
struct RuntimeState {
    /// Unix timestamp of the last manual trigger, or `None` if it has never been triggered.
    #[serde(default)]
    last_manual_trigger_at: Option<u64>,
}

/// Scheduler-written runtime state for a routine, persisted to the gitignored
/// `scheduled.local.toml` sidecar.
///
/// Written by the routine's launch command (the `printf` step of `build_routine_command`) at each
/// scheduled cron firing and only ever read here — kept separate from [`RuntimeState`] so a
/// daemon-side re-persist of `state.local.toml` can never clobber the scheduler's timestamp.
#[derive(Debug, Deserialize, Serialize)]
struct ScheduledState {
    /// Unix timestamp of the last scheduled (cron) firing, or `None` if it has never fired.
    #[serde(default)]
    last_scheduled_trigger_at: Option<u64>,
}

/// Parse a routine TOML file at `path`, returning `None` on any error.
fn read_routine_toml(path: &std::path::PathBuf) -> Option<RoutineToml> {
    let text = std::fs::read_to_string(path).ok()?;
    toml::from_str(&text).ok()
}

/// Read `last_manual_trigger_at` from a routine's `state.local.toml` sidecar, returning `None` when
/// the sidecar is absent or unparsable (e.g. before the routine has ever been triggered).
fn read_runtime_state(dir_name: &str) -> Option<u64> {
    let text = std::fs::read_to_string(routine_state_path(dir_name)).ok()?;
    toml::from_str::<RuntimeState>(&text)
        .ok()?
        .last_manual_trigger_at
}

/// Read `last_scheduled_trigger_at` from a routine's `scheduled.local.toml` sidecar, returning
/// `None` when the sidecar is absent or unparsable (e.g. before the routine's schedule has ever
/// fired). The daemon only reads this file; it is written by the routine's launch command at fire
/// time.
fn read_scheduled_state(dir_name: &str) -> Option<u64> {
    let text = std::fs::read_to_string(routine_scheduled_state_path(dir_name)).ok()?;
    toml::from_str::<ScheduledState>(&text)
        .ok()?
        .last_scheduled_trigger_at
}

/// Load a routine from `{routines_dir}/{dir_name}/routine.toml`.
///
/// `dir_name` is the slug (title-derived folder name). The routine's UUID `id` is read from
/// `routine.toml`; for legacy dirs created before this change `id` falls back to `dir_name`.
///
/// `last_manual_trigger_at` is read from the `state.local.toml` sidecar, falling back to the legacy
/// `routine.toml` field for routines written before the runtime state was split out.
fn load_routine_from_dir(dir_name: &str) -> Option<Routine> {
    let toml = read_routine_toml(&routine_toml_path(dir_name))?;
    let title = toml.title?;
    let id = toml.id.unwrap_or_else(|| dir_name.to_string());
    let last_manual_trigger_at = read_runtime_state(dir_name).or(toml.last_manual_trigger_at);
    let last_scheduled_trigger_at = read_scheduled_state(dir_name);
    Some(Routine {
        id,
        schedule: toml.schedule?,
        title,
        agent: toml.agent?,
        prompt: toml.prompt.unwrap_or_default(),
        repositories: toml.repositories,
        enabled: toml.enabled.unwrap_or(true),
        source: "managed".to_string(),
        created_at: toml.created_at.unwrap_or(0),
        updated_at: toml.updated_at.unwrap_or(0),
        last_manual_trigger_at,
        last_scheduled_trigger_at,
        ttl_secs: toml.ttl_secs,
        max_runtime_secs: toml.max_runtime_secs,
    })
}

/// Write `routine` to disk: `routine.toml` (tracked config), the composed `prompt.md`, the
/// gitignored `state.local.toml` runtime sidecar, and `.gitignore` if absent.
///
/// The folder is named after the slugified title (`slugify(&routine.title)`). The UUID `id` is
/// stored inside `routine.toml` so it survives a rename. Daemon-written runtime state
/// (`last_manual_trigger_at`) goes to the sidecar, not `routine.toml`, so a trigger never churns the
/// version-controlled config file.
pub fn write_routine(routine: &Routine) -> std::io::Result<()> {
    let slug = slugify(&routine.title);
    let dir = routine_dir(&slug);
    std::fs::create_dir_all(&dir)?;

    let gitignore = routine_gitignore_path(&slug);
    if !gitignore.exists() {
        std::fs::write(&gitignore, "*.local.*\n*.log\nrun.sh\n")?;
    }

    // Remove any stale `run.sh` left by an older daemon that generated per-routine launch scripts;
    // the crontab line now invokes the binary directly, so the script is obsolete. Best-effort: a
    // missing file is fine. Startup re-persists every routine, so this heals existing installs.
    let _ = std::fs::remove_file(routine_script_path(&slug));

    let toml_routine = RoutineToml {
        id: Some(routine.id.clone()),
        schedule: Some(routine.schedule.clone()),
        title: Some(routine.title.clone()),
        agent: Some(routine.agent.clone()),
        prompt: Some(routine.prompt.clone()),
        repositories: routine.repositories.clone(),
        enabled: Some(routine.enabled),
        created_at: Some(routine.created_at),
        updated_at: Some(routine.updated_at),
        // Runtime state is written to the sidecar below, never to the tracked `routine.toml`
        // (`skip_serializing` also keeps this field out regardless of its value).
        last_manual_trigger_at: None,
        ttl_secs: routine.ttl_secs,
        max_runtime_secs: routine.max_runtime_secs,
    };
    let text = toml::to_string_pretty(&toml_routine).map_err(std::io::Error::other)?;
    // Atomic write (temp + rename) so any concurrent reader never observes a torn routine.toml —
    // a torn file parses to `None` and would silently drop the routine from the store. (Note:
    // there is no continuously-running reverse crontab sync re-reading these files; reverse sync
    // is implemented but not wired up — see issue #218.)
    atomic_write(&routine_toml_path(&slug), text.as_bytes())?;
    atomic_write(
        &routine_prompt_path(&slug),
        compose_prompt(routine).as_bytes(),
    )?;
    write_runtime_state(&slug, routine.last_manual_trigger_at)?;
    Ok(())
}

/// Persist a routine's runtime state to its gitignored `state.local.toml` sidecar.
///
/// Writes the sidecar (atomically) when `last_manual_trigger_at` is set, and removes any stale
/// sidecar when it is `None`, so the on-disk state always mirrors the in-memory routine.
fn write_runtime_state(slug: &str, last_manual_trigger_at: Option<u64>) -> std::io::Result<()> {
    let path = routine_state_path(slug);
    match last_manual_trigger_at {
        Some(_) => {
            let state = RuntimeState {
                last_manual_trigger_at,
            };
            let text = toml::to_string_pretty(&state).map_err(std::io::Error::other)?;
            atomic_write(&path, text.as_bytes())?;
        }
        None => {
            if path.exists() {
                std::fs::remove_file(&path)?;
            }
        }
    }
    Ok(())
}

/// Remove the directory for a routine identified by its slug, doing nothing if it does not exist.
pub fn remove_routine_dir(slug: &str) -> std::io::Result<()> {
    let dir = routine_dir(slug);
    if dir.exists() {
        std::fs::remove_dir_all(dir)?;
    }
    Ok(())
}

/// Rename any `prompt.txt` sidecar to `prompt.md` in every routine directory.
///
/// Call once at startup before syncing the crontab. Routines written by older daemon versions have
/// `prompt.txt` on disk; the new `run.sh` references `prompt.md`, so the first cron trigger would
/// fail the `cp` step if this migration has not run.
pub fn migrate_prompt_files() {
    migrate_prompt_files_from_dir(&routines_dir());
}

/// Inner variant of [`migrate_prompt_files`] that scans `dir` instead of [`routines_dir`].
///
/// Extracted so tests can drive the migration against a controlled scratch directory, including the
/// `read_dir` error-return branch and the per-entry rename-failure branch.
pub(crate) fn migrate_prompt_files_from_dir(dir: &std::path::Path) {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        if !entry.file_type().is_ok_and(|ft| ft.is_dir()) {
            continue;
        }
        let old = entry.path().join("prompt.txt");
        let new = entry.path().join("prompt.md");
        if old.exists() && !new.exists() {
            if let Err(err) = std::fs::rename(&old, &new) {
                log::warn!("migrate_prompt_files: failed to rename {old:?}: {err}");
            }
        }
    }
}

/// Migrate legacy UUID-named routine directories to the current slug-based layout.
///
/// Early daemon versions stored each routine under `{routines_dir}/{id}/` (the UUID). The current
/// layout uses `{routines_dir}/{slugify(title)}/`. After an upgrade the legacy dir still holds the
/// real `routine.toml` + `prompt.md`, while the crontab sync creates a *fresh* slug dir containing
/// only `run.sh` — so the cron `cp prompt.md` reads an empty dir and the agent launches task-less.
///
/// For every on-disk routine whose directory name does not already equal its slug, this re-persists
/// it into the slug dir (preserving any `run.sh` already there) and removes the stale legacy dir.
/// Idempotent: routines already in their slug dir are skipped. Call once at startup before
/// `load_store` so the in-memory store reflects the canonical layout.
pub fn migrate_routine_dirs() {
    migrate_routine_dirs_from_dir(&routines_dir());
}

/// Inner variant of [`migrate_routine_dirs`] that scans `dir` instead of [`routines_dir`].
///
/// Extracted so tests can drive the migration against a controlled scratch directory, exercising the
/// `read_dir` error-return branch, the non-directory and unparsable-toml `continue` branches, and the
/// `write_routine`/`remove_routine_dir` failure-log branches.
pub(crate) fn migrate_routine_dirs_from_dir(dir: &std::path::Path) {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        if !entry.file_type().is_ok_and(|ft| ft.is_dir()) {
            continue;
        }
        let dir_name = entry.file_name().to_string_lossy().to_string();
        let Some(routine) = load_routine_from_dir(&dir_name) else {
            // A dir without a parsable routine.toml (e.g. a sync-created dir holding only run.sh)
            // carries no routine to migrate; the routine it shadows is healed from its own dir.
            continue;
        };
        let slug = slugify(&routine.title);
        if slug == dir_name {
            continue;
        }
        if let Err(err) = write_routine(&routine) {
            log::warn!("migrate_routine_dirs: failed to write {slug:?}: {err}; leaving legacy dir");
            continue;
        }
        if let Err(err) = remove_routine_dir(&dir_name) {
            log::warn!("migrate_routine_dirs: failed to remove legacy dir {dir_name:?}: {err}");
        }
    }
}

/// Re-persist every loaded routine to disk, recreating `routine.toml`, `prompt.md`, and `.gitignore`
/// in its canonical slug directory.
///
/// Nothing else rewrites the prompt sidecar on startup, so a slug dir missing its `prompt.md` (e.g.
/// after the UUID→slug migration, or if the sidecar was lost) would fail the launch command's
/// `cp prompt.md`. Re-persisting from the in-memory store heals those dirs (and removes any stale
/// legacy `run.sh`). Idempotent; safe to call on every startup after [`load_store`].
pub fn repersist_routines(store: &RoutineStore) {
    let routines: Vec<Routine> = store.lock_recover().values().cloned().collect();
    for routine in &routines {
        if let Err(err) = write_routine(routine) {
            log::warn!(
                "repersist_routines: failed to write routine {:?}: {err}",
                routine.id
            );
        }
    }
}

/// Scan `~/.config/moadim/routines/` and load all valid routines into a new store.
pub fn load_store() -> RoutineStore {
    load_store_from_dir(&routines_dir())
}

/// Scan `dir` and load all valid routines into a new store.
pub(crate) fn load_store_from_dir(dir: &std::path::Path) -> RoutineStore {
    let mut routines = HashMap::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            if entry.file_type().is_ok_and(|ft| ft.is_dir()) {
                let dir_name = entry.file_name().to_string_lossy().to_string();
                if let Some(routine) = load_routine_from_dir(&dir_name) {
                    routines.insert(routine.id.clone(), routine);
                }
            }
        }
    }
    Arc::new(Mutex::new(routines))
}

#[cfg(test)]
#[path = "routine_storage_tests.rs"]
mod routine_storage_tests;
