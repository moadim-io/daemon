//! TOML-backed persistence for routines, plus the tracked `schedule.cron` and the
//! `prompts/prompt.pure.md` (raw) / `prompts/prompt.compiled.local.md` (composed) sidecar files.

use crate::utils::lock::LockRecover;
// Re-exported (as `super::routines_dir`) for `routine_storage_migrations`; not called directly
// in this file since `load_store`/`load_store_from_dir` moved to `routine_storage_load`.
use crate::paths::routines_dir;
// Only referenced by the `#[cfg(test)]` sibling test modules via `use super::*;` (they build a
// `RoutineStore` map by hand); not used directly in this file's own (non-test) code.
#[cfg(test)]
use std::collections::HashMap;
#[cfg(test)]
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};

use crate::paths::{
    routine_compiled_prompt_path, routine_cron_path, routine_dir, routine_gitignore_path,
    routine_manual_log_path, routine_prompts_dir, routine_pure_prompt_path, routine_script_path,
    routine_skip_log_path, routine_state_path, routine_toml_path,
};
use crate::routines::{compose_prompt, slugify, Repository, Routine, RoutineStore};
use crate::utils::atomic::atomic_write;

/// TOML representation of a routine on disk.
#[derive(Debug, Deserialize, Serialize)]
struct RoutineToml {
    /// UUID that uniquely identifies this routine (stable across renames).
    id: Option<String>,
    /// Cron expression.
    ///
    /// **Read-only / legacy.** The schedule now lives in the tracked `schedule.cron` sidecar so
    /// it stays diff-friendly and smaller than the rest of the routine metadata.
    #[serde(default, skip_serializing)]
    schedule: Option<String>,
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
    /// Free-form labels for the routine; absent means no tags.
    #[serde(default)]
    tags: Vec<String>,
}

/// Daemon-written runtime state for a routine, persisted to the gitignored `state.local.toml`
/// sidecar so it never appears in the version-controlled `routine.toml`.
///
/// Trigger history (`last_manual_trigger_at`, `last_scheduled_trigger_at`) is no longer stored
/// here — it lives in the append-only `manual.log` / `scheduled.log` files instead.
#[derive(Debug, Default, Deserialize, Serialize)]
struct RuntimeState {
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
}

/// Parse a routine TOML file at `path`, returning `None` on any error.
fn read_routine_toml(path: &std::path::PathBuf) -> Option<RoutineToml> {
    let text = std::fs::read_to_string(path).ok()?;
    toml::from_str(&text).ok()
}

/// Read a routine's tracked cron entry from `schedule.cron`, returning the first non-empty line.
fn read_routine_cron(path: &std::path::PathBuf) -> Option<String> {
    let text = std::fs::read_to_string(path).ok()?;
    let schedule = text.lines().find(|line| !line.trim().is_empty())?.trim();
    Some(schedule.to_string())
}

/// Read a routine's `state.local.toml` sidecar under `base`, defaulting to an empty
/// [`RuntimeState`] when the sidecar is absent or unparsable (e.g. before the routine has ever
/// been snoozed).
///
/// Base-dir-aware so the `routine_storage_load` loaders can resolve it coherently for any scan
/// root, not only the global [`routines_dir`].
fn read_runtime_state(base: &std::path::Path, dir_name: &str) -> RuntimeState {
    std::fs::read_to_string(base.join(dir_name).join("state.local.toml"))
        .ok()
        .and_then(|text| toml::from_str(&text).ok())
        .unwrap_or_default()
}

/// Patterns every routine's `.gitignore` must carry: machine-local runtime state, logs, and the
/// obsolete per-routine launch script. `prompts/prompt.compiled.local.md` needs no entry of its
/// own — its `.local.` filename already matches `*.local.*` (issue #1046).
const ROUTINE_GITIGNORE_REQUIRED: &[&str] = &["*.local.*", "*.log", "run.sh"];

/// Ensure `path` (a routine's `.gitignore`) contains every pattern in [`ROUTINE_GITIGNORE_REQUIRED`],
/// appending whichever are missing and leaving the rest of the file (including user additions)
/// untouched. Mirrors `cli_system::ensure_config_gitignore`'s reconciliation, scoped per routine.
/// [`write_routine`] calls this unconditionally, so [`repersist_routines`] heals existing installs'
/// `.gitignore` files on every daemon startup, not just newly created ones.
fn ensure_routine_gitignore(path: &std::path::Path) -> std::io::Result<()> {
    let existing = std::fs::read_to_string(path).unwrap_or_default();
    let lines: Vec<&str> = existing.lines().collect();
    let missing: Vec<&str> = ROUTINE_GITIGNORE_REQUIRED
        .iter()
        .copied()
        .filter(|pat| !lines.iter().any(|line| line.trim() == *pat))
        .collect();
    if missing.is_empty() {
        return Ok(());
    }
    let mut content = existing;
    if !content.is_empty() && !content.ends_with('\n') {
        content.push('\n');
    }
    for pattern in &missing {
        content.push_str(pattern);
        content.push('\n');
    }
    std::fs::write(path, &content)
}

/// Write `routine` to disk: `routine.toml` (tracked config), `schedule.cron` (tracked cron entry),
/// the `prompts/prompt.pure.md` (raw) and `prompts/prompt.compiled.local.md` (composed) sidecars,
/// the gitignored `state.local.toml` runtime sidecar, and `.gitignore` (created or reconciled — see
/// [`ensure_routine_gitignore`]).
///
/// The folder is named after the slugified title (`slugify(&routine.title)`). The UUID `id` is
/// stored inside `routine.toml` so it survives a rename. Daemon-written runtime state
/// (`last_manual_trigger_at`) goes to the sidecar, not `routine.toml`, so a trigger never churns the
/// version-controlled config file.
///
/// Two distinct titles can slugify to the same folder name (e.g. `"Update deps!"` and
/// `"Update deps?"` both become `update-deps`). In-memory create/update handlers already reject
/// that when both routines are loaded in the [`RoutineStore`], but a slug can also collide with a
/// stale on-disk `routine.toml` that isn't (or is no longer) in memory — e.g. a directory left
/// behind by a failed [`remove_routine_dir`]. Guard here too, as the last line of defense against
/// silently overwriting another routine's files (#188): refuse to write when the target slug's
/// `routine.toml` already exists and belongs to a different `id`.
pub fn write_routine(routine: &Routine) -> std::io::Result<()> {
    let slug = slugify(&routine.title);
    let dir = routine_dir(&slug);
    if let Some(existing_id) =
        read_routine_toml(&routine_toml_path(&slug)).and_then(|existing| existing.id)
    {
        if existing_id != routine.id {
            return Err(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                format!(
                    "slug \"{slug}\" is already used on disk by routine {existing_id}; refusing to overwrite it"
                ),
            ));
        }
    }
    crate::utils::fs_perms::create_private_dir_all(&dir)?;
    crate::utils::fs_perms::create_private_dir_all(&routine_prompts_dir(&slug))?;

    ensure_routine_gitignore(&routine_gitignore_path(&slug))?;

    // Remove any stale `run.sh` left by an older daemon that generated per-routine launch scripts;
    // the crontab line now invokes the binary directly, so the script is obsolete. Best-effort: a
    // missing file is fine. Startup re-persists every routine, so this heals existing installs.
    let _ = std::fs::remove_file(routine_script_path(&slug));

    let toml_routine = RoutineToml {
        id: Some(routine.id.clone()),
        schedule: None,
        title: Some(routine.title.clone()),
        agent: Some(routine.agent.clone()),
        model: routine.model.clone(),
        // Never written; the raw prompt now lives in the `prompts/prompt.pure.md` sidecar below.
        prompt: None,
        goal: routine.goal.clone(),
        repositories: routine.repositories.clone(),
        machines: routine.machines.clone(),
        enabled: Some(routine.enabled),
        created_at: Some(routine.created_at),
        updated_at: Some(routine.updated_at),
        // Runtime state is written to the sidecar below, never to the tracked `routine.toml`
        // (`skip_serializing` also keeps this field out regardless of its value).
        last_manual_trigger_at: None,
        ttl_secs: routine.ttl_secs,
        max_runtime_secs: routine.max_runtime_secs,
        tags: routine.tags.clone(),
    };
    let text = toml::to_string_pretty(&toml_routine).map_err(std::io::Error::other)?;
    // Atomic write (temp + rename) so any concurrent reader never observes a torn routine.toml —
    // a torn file parses to `None` and would silently drop the routine from the store. (Note:
    // there is no continuously-running reverse crontab sync re-reading these files; reverse sync
    // is implemented but not wired up — see issue #218.)
    atomic_write(&routine_toml_path(&slug), text.as_bytes())?;
    atomic_write(
        &routine_cron_path(&slug),
        format!("{}\n", routine.schedule).as_bytes(),
    )?;
    atomic_write(&routine_pure_prompt_path(&slug), routine.prompt.as_bytes())?;
    atomic_write(
        &routine_compiled_prompt_path(&slug),
        compose_prompt(routine).as_bytes(),
    )?;
    write_runtime_state(&slug, routine)?;
    Ok(())
}

/// Persist a routine's runtime state to its gitignored `state.local.toml` sidecar.
///
/// Writes the sidecar (atomically) when any tracked field is set, and removes any stale sidecar
/// when all are `None`, so the on-disk state always mirrors the in-memory routine.
fn write_runtime_state(slug: &str, routine: &Routine) -> std::io::Result<()> {
    let path = routine_state_path(slug);
    if routine.snoozed_until.is_none() && routine.skip_runs.is_none() && !routine.power_saving {
        if path.exists() {
            std::fs::remove_file(&path)?;
        }
        return Ok(());
    }
    let state = RuntimeState {
        // Not written (skip_serializing); stored only to satisfy the struct; reads migrate the
        // legacy value from here into manual.log on first load.
        last_manual_trigger_at: None,
        snoozed_until: routine.snoozed_until,
        skip_runs: routine.skip_runs,
        power_saving: routine.power_saving,
    };
    let text = toml::to_string_pretty(&state).map_err(std::io::Error::other)?;
    atomic_write(&path, text.as_bytes())?;
    Ok(())
}

/// Append a Unix-timestamp entry to a routine's `manual.log`, recording a manual trigger.
///
/// Called by `svc_trigger` immediately after stamping `last_manual_trigger_at` on the in-memory
/// routine. Best-effort: a log-write failure is warned but never surfaced to the caller, so a
/// disk hiccup can't block the trigger itself.
pub fn append_manual_trigger_log(slug: &str, ts: u64) {
    let path = routine_manual_log_path(slug);
    let line = format!("{ts}\n");
    if let Err(err) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .and_then(|mut file| std::io::Write::write_all(&mut file, line.as_bytes()))
    {
        log::warn!(
            "append_manual_trigger_log: failed to write {}: {err}",
            path.display()
        );
    }
}

/// Append a `{ts}\t{reason}` entry to a routine's `skip.log`, recording why a trigger did not
/// spawn a workbench.
///
/// Called by `spawn_routine_command` from every branch that returns without launching (agent load
/// failure, an oversized inline prompt, the overlap guard, or the global concurrency cap), so
/// `routine_logs` has something to show instead of coming back empty when the newest — or only —
/// signal for a skipped trigger previously lived solely in the daemon's own process log (#1145).
/// Best-effort, like [`append_manual_trigger_log`]: a log-write failure is warned but never
/// surfaced, so a disk hiccup can't turn a skip into a harder failure.
pub fn append_skip_log(slug: &str, ts: u64, reason: &str) {
    let path = routine_skip_log_path(slug);
    let line = format!("{ts}\t{reason}\n");
    if let Err(err) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .and_then(|mut file| std::io::Write::write_all(&mut file, line.as_bytes()))
    {
        log::warn!("append_skip_log: failed to write {}: {err}", path.display());
    }
}

/// Remove the directory for a routine identified by its slug, doing nothing if it does not exist.
pub fn remove_routine_dir(slug: &str) -> std::io::Result<()> {
    let dir = routine_dir(slug);
    if dir.exists() {
        std::fs::remove_dir_all(dir)?;
    }
    Ok(())
}

/// Re-persist every loaded routine to disk, recreating `routine.toml`, `schedule.cron`,
/// `prompts/prompt.pure.md`, `prompts/prompt.compiled.local.md`, and `.gitignore` in its canonical
/// slug directory.
///
/// Nothing else rewrites the prompt sidecars on startup, so a slug dir missing its
/// `prompts/prompt.compiled.local.md` (e.g. after the UUID→slug migration, or if the sidecar was
/// lost) would fail the launch command's `cp prompt.compiled.local.md`. Re-persisting from the
/// in-memory store
/// heals those dirs (and removes any stale legacy `run.sh`). Idempotent; safe to call on every
/// startup after [`load_store`].
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

#[path = "routine_storage_load.rs"]
mod routine_storage_load;
use routine_storage_load::load_routine_from_dir;
pub use routine_storage_load::load_store;
#[cfg(test)]
pub(crate) use routine_storage_load::load_store_from_dir;
pub(crate) use routine_storage_load::reload_store_from_dir;

#[path = "routine_storage_migrations.rs"]
mod routine_storage_migrations;
pub use routine_storage_migrations::{
    migrate_compiled_prompt_filename, migrate_prompt_files, migrate_prompts_to_subfolder,
    migrate_routine_dirs, migrate_trigger_logs,
};
#[cfg(test)]
pub(crate) use routine_storage_migrations::{
    migrate_compiled_prompt_filename_from_dir, migrate_prompt_files_from_dir,
    migrate_prompts_to_subfolder_from_dir, migrate_routine_dirs_from_dir,
    migrate_trigger_logs_from_dir,
};

#[cfg(test)]
#[path = "routine_storage_tests.rs"]
mod routine_storage_tests;

#[cfg(test)]
#[path = "routine_storage_prompt_sidecar_tests.rs"]
mod routine_storage_prompt_sidecar_tests;

#[cfg(test)]
#[path = "routine_storage_migration_tests.rs"]
mod routine_storage_migration_tests;

#[cfg(test)]
#[path = "routine_storage_prompt_file_migration_tests.rs"]
mod routine_storage_prompt_file_migration_tests;

#[cfg(test)]
#[path = "routine_storage_compiled_prompt_migration_tests.rs"]
mod routine_storage_compiled_prompt_migration_tests;

#[cfg(test)]
#[path = "routine_storage_snooze_tests.rs"]
mod routine_storage_snooze_tests;

#[cfg(test)]
#[path = "routine_storage_trigger_log_migration_tests.rs"]
mod routine_storage_trigger_log_migration_tests;

#[cfg(test)]
#[path = "routine_storage_sidecar_state_tests.rs"]
mod routine_storage_sidecar_state_tests;

#[cfg(test)]
#[path = "routine_storage_slug_collision_tests.rs"]
mod routine_storage_slug_collision_tests;
