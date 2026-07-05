//! TOML-backed persistence for routines, plus the `prompts/prompt.pure.md` (raw) and
//! `prompts/prompt.compiled.md` (composed) sidecar files.

use crate::utils::lock::LockRecover;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};

use crate::paths::{
    routine_compiled_prompt_path, routine_dir, routine_gitignore_path, routine_manual_log_path,
    routine_prompts_dir, routine_pure_prompt_path, routine_scheduled_log_path, routine_script_path,
    routine_state_path, routine_toml_path, routines_dir,
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

/// Legacy scheduled-state TOML, superseded by the `scheduled.log` append-only file.
///
/// Only used during startup migration: if `scheduled.local.toml` exists and `scheduled.log` does
/// not, the stored timestamp is seeded as the first log entry and the TOML file is removed.
#[derive(Debug, Deserialize, Serialize)]
struct LegacyScheduledState {
    /// Unix timestamp of the last scheduled (cron) firing stored in the superseded TOML format.
    #[serde(default)]
    last_scheduled_trigger_at: Option<u64>,
}

/// Parse a routine TOML file at `path`, returning `None` on any error.
fn read_routine_toml(path: &std::path::PathBuf) -> Option<RoutineToml> {
    let text = std::fs::read_to_string(path).ok()?;
    toml::from_str(&text).ok()
}

/// Read a routine's `state.local.toml` sidecar, defaulting to an empty [`RuntimeState`] when the
/// sidecar is absent or unparsable (e.g. before the routine has ever been snoozed).
fn read_runtime_state(dir_name: &str) -> RuntimeState {
    std::fs::read_to_string(routine_state_path(dir_name))
        .ok()
        .and_then(|text| toml::from_str(&text).ok())
        .unwrap_or_default()
}

/// Read the last Unix-timestamp line from an append-only trigger log (e.g. `scheduled.log` or
/// `manual.log`), returning `None` when the file is absent or contains no parsable timestamp.
fn read_last_log_timestamp(path: &std::path::Path) -> Option<u64> {
    let text = std::fs::read_to_string(path).ok()?;
    text.lines()
        .rev()
        .find_map(|line| line.trim().parse::<u64>().ok())
}

/// Read `last_scheduled_trigger_at` from a routine's `scheduled.log`, returning `None` when the
/// log is absent or empty (e.g. before the routine's schedule has ever fired).
fn read_scheduled_state(dir_name: &str) -> Option<u64> {
    read_last_log_timestamp(&routine_scheduled_log_path(dir_name))
}

/// Read `last_manual_trigger_at` from a routine's `manual.log`, returning `None` when the log is
/// absent or empty.
fn read_manual_state(dir_name: &str) -> Option<u64> {
    read_last_log_timestamp(&routine_manual_log_path(dir_name))
}

/// Read a routine's raw prompt from its `prompts/prompt.pure.md` sidecar, falling back to the
/// legacy `routine.toml` `prompt` field for a dir that has not been migrated yet.
fn read_pure_prompt(dir_name: &str, legacy: Option<String>) -> String {
    std::fs::read_to_string(routine_pure_prompt_path(dir_name))
        .ok()
        .or(legacy)
        .unwrap_or_default()
}

/// Load a routine from `{routines_dir}/{dir_name}/routine.toml`.
///
/// `dir_name` is the slug (title-derived folder name). The routine's UUID `id` is read from
/// `routine.toml`; for legacy dirs created before this change `id` falls back to `dir_name`.
///
/// `last_manual_trigger_at`, `snoozed_until`, and `skip_runs` are read from the `state.local.toml`
/// sidecar; `last_manual_trigger_at` falls back to the legacy `routine.toml` field for routines
/// written before the runtime state was split out.
fn load_routine_from_dir(dir_name: &str) -> Option<Routine> {
    let toml = read_routine_toml(&routine_toml_path(dir_name))?;
    let title = toml.title?;
    let id = toml.id.unwrap_or_else(|| dir_name.to_string());
    let runtime_state = read_runtime_state(dir_name);
    // Prefer the log file; fall back to legacy state.local.toml field then routine.toml field for
    // routines that predate the log-file migration.
    let last_manual_trigger_at = read_manual_state(dir_name)
        .or(runtime_state.last_manual_trigger_at)
        .or(toml.last_manual_trigger_at);
    let last_scheduled_trigger_at = read_scheduled_state(dir_name);
    let prompt = read_pure_prompt(dir_name, toml.prompt);
    Some(Routine {
        id,
        schedule: toml.schedule?,
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
    })
}

/// Write `routine` to disk: `routine.toml` (tracked config), the `prompts/prompt.pure.md` (raw) and
/// `prompts/prompt.compiled.md` (composed) sidecars, the gitignored `state.local.toml` runtime
/// sidecar, and `.gitignore` if absent.
///
/// The folder is named after the slugified title (`slugify(&routine.title)`). The UUID `id` is
/// stored inside `routine.toml` so it survives a rename. Daemon-written runtime state
/// (`last_manual_trigger_at`) goes to the sidecar, not `routine.toml`, so a trigger never churns the
/// version-controlled config file.
pub fn write_routine(routine: &Routine) -> std::io::Result<()> {
    let slug = slugify(&routine.title);
    let dir = routine_dir(&slug);
    std::fs::create_dir_all(&dir)?;
    std::fs::create_dir_all(routine_prompts_dir(&slug))?;

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
    let text = toml::to_string_pretty(&toml_routine).expect(
        "RoutineToml serialization cannot fail for a struct with only primitive and Option fields",
    );
    // Atomic write (temp + rename) so any concurrent reader never observes a torn routine.toml —
    // a torn file parses to `None` and would silently drop the routine from the store. (Note:
    // there is no continuously-running reverse crontab sync re-reading these files; reverse sync
    // is implemented but not wired up — see issue #218.)
    atomic_write(&routine_toml_path(&slug), text.as_bytes())?;
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
    let text = toml::to_string_pretty(&state)
        .expect("RuntimeState serialization cannot fail for a struct with only Option fields");
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

/// Migrate per-routine trigger state from legacy TOML sidecars to append-only log files.
///
/// For each routine directory:
/// - If `scheduled.local.toml` exists and `scheduled.log` does not, the stored timestamp is
///   written as the first log line and the TOML file is removed.
/// - If `state.local.toml` contains a `last_manual_trigger_at` field and `manual.log` does not
///   exist, the stored timestamp is written as the first log line.
///
/// Call once at startup, after [`migrate_prompt_files`] and before [`load_store`].
pub fn migrate_trigger_logs() {
    migrate_trigger_logs_from_dir(&routines_dir());
}

/// Inner variant of [`migrate_trigger_logs`] that scans `dir` instead of [`routines_dir`].
pub(crate) fn migrate_trigger_logs_from_dir(dir: &std::path::Path) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        if !entry.file_type().is_ok_and(|ft| ft.is_dir()) {
            continue;
        }
        let routine_dir = entry.path();

        // Migrate scheduled.local.toml → scheduled.log
        let old_sched = routine_dir.join("scheduled.local.toml");
        let new_sched = routine_dir.join("scheduled.log");
        if old_sched.exists() && !new_sched.exists() {
            if let Some(ts) = std::fs::read_to_string(&old_sched)
                .ok()
                .and_then(|text| toml::from_str::<LegacyScheduledState>(&text).ok())
                .and_then(|state| state.last_scheduled_trigger_at)
            {
                let line = format!("{ts}\n");
                if let Err(err) = std::fs::write(&new_sched, line.as_bytes()) {
                    log::warn!(
                        "migrate_trigger_logs: failed to write {}: {err}",
                        new_sched.display()
                    );
                    continue;
                }
            }
            let _ = std::fs::remove_file(&old_sched);
        }

        // Migrate last_manual_trigger_at from state.local.toml → manual.log
        let new_manual = routine_dir.join("manual.log");
        if !new_manual.exists() {
            if let Some(ts) = std::fs::read_to_string(routine_dir.join("state.local.toml"))
                .ok()
                .and_then(|text| toml::from_str::<RuntimeState>(&text).ok())
                .and_then(|state| state.last_manual_trigger_at)
            {
                let line = format!("{ts}\n");
                if let Err(err) = std::fs::write(&new_manual, line.as_bytes()) {
                    log::warn!(
                        "migrate_trigger_logs: failed to write {}: {err}",
                        new_manual.display()
                    );
                }
            }
        }
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
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        if !entry.file_type().is_ok_and(|ft| ft.is_dir()) {
            continue;
        }
        let old = entry.path().join("prompt.txt");
        let new = entry.path().join("prompt.md");
        if old.exists() && !new.exists() {
            if let Err(err) = std::fs::rename(&old, &new) {
                log::warn!(
                    "migrate_prompt_files: failed to rename {}: {err}",
                    old.display()
                );
            }
        }
    }
}

/// Move each routine's prompt file(s) into its `prompts/` subfolder, and extract the raw prompt out
/// of `routine.toml` into `prompts/prompt.pure.md`.
///
/// Call once at startup, after [`migrate_prompt_files`] (which renames `prompt.txt` to `prompt.md`)
/// and before [`migrate_routine_dirs`] / `load_store`. Older daemons wrote a single top-level
/// `prompt.md` (the composed prompt) and kept the raw prompt inside `routine.toml`'s `prompt` field;
/// this daemon reads the raw prompt from `prompts/prompt.pure.md` and the composed prompt from
/// `prompts/prompt.compiled.md`, so an un-migrated dir would launch with an empty prompt.
pub fn migrate_prompts_to_subfolder() {
    migrate_prompts_to_subfolder_from_dir(&routines_dir());
}

/// Inner variant of [`migrate_prompts_to_subfolder`] that scans `dir` instead of [`routines_dir`].
///
/// Extracted so tests can drive the migration against a controlled scratch directory, including the
/// `read_dir` error-return branch and the per-entry rename/write-failure branches.
pub(crate) fn migrate_prompts_to_subfolder_from_dir(dir: &std::path::Path) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        if !entry.file_type().is_ok_and(|ft| ft.is_dir()) {
            continue;
        }
        // Skip dirs with no `routine.toml` at all: not a routine (e.g. an orphaned/leftover dir),
        // so there is nothing to migrate. Without this guard the migration resurrects an empty
        // `prompts/prompt.pure.md` sidecar in such dirs on every startup.
        if !entry.path().join("routine.toml").exists() {
            continue;
        }
        let prompts_dir = entry.path().join("prompts");
        if let Err(err) = std::fs::create_dir_all(&prompts_dir) {
            log::warn!(
                "migrate_prompts_to_subfolder: failed to create {}: {err}",
                prompts_dir.display()
            );
            continue;
        }

        let old_compiled = entry.path().join("prompt.md");
        let new_compiled = prompts_dir.join("prompt.compiled.md");
        if old_compiled.exists() && !new_compiled.exists() {
            if let Err(err) = std::fs::rename(&old_compiled, &new_compiled) {
                log::warn!(
                    "migrate_prompts_to_subfolder: failed to rename {}: {err}",
                    old_compiled.display()
                );
            }
        }

        let pure = prompts_dir.join("prompt.pure.md");
        if !pure.exists() {
            let legacy_prompt = read_routine_toml(&entry.path().join("routine.toml"))
                .and_then(|toml| toml.prompt)
                .unwrap_or_default();
            if let Err(err) = std::fs::write(&pure, legacy_prompt.as_bytes()) {
                log::warn!(
                    "migrate_prompts_to_subfolder: failed to write {}: {err}",
                    pure.display()
                );
            }
        }
    }
}

/// Migrate legacy UUID-named routine directories to the current slug-based layout.
///
/// Early daemon versions stored each routine under `{routines_dir}/{id}/` (the UUID). The current
/// layout uses `{routines_dir}/{slugify(title)}/`. After an upgrade the legacy dir still holds the
/// real `routine.toml` + `prompts/prompt.compiled.md`, while the crontab sync creates a *fresh* slug
/// dir containing only `run.sh` — so the cron `cp prompt.compiled.md` reads an empty dir and the
/// agent launches task-less.
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
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
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

/// Re-persist every loaded routine to disk, recreating `routine.toml`, `prompts/prompt.pure.md`,
/// `prompts/prompt.compiled.md`, and `.gitignore` in its canonical slug directory.
///
/// Nothing else rewrites the prompt sidecars on startup, so a slug dir missing its
/// `prompts/prompt.compiled.md` (e.g. after the UUID→slug migration, or if the sidecar was lost)
/// would fail the launch command's `cp prompt.compiled.md`. Re-persisting from the in-memory store
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
                match load_routine_from_dir(&dir_name) {
                    Some(routine) => {
                        routines.insert(routine.id.clone(), routine);
                    }
                    // A dir whose routine.toml exists but the loader rejected (unparsable, or
                    // missing a required field) would otherwise vanish from the store, UI, API
                    // and crontab with no trace. Warn so the operator can find and fix the file
                    // instead of hunting a routine that silently disappeared.
                    None if routine_toml_path(&dir_name).exists() => {
                        log::warn!(
                            "load_store: skipping routine dir {dir_name:?}: its routine.toml is \
                             unparsable or missing a required field (title, schedule, or agent)"
                        );
                    }
                    // No routine.toml at all — not a routine dir; skip it quietly.
                    None => {}
                }
            }
        }
    }
    Arc::new(Mutex::new(routines))
}

#[cfg(test)]
#[path = "routine_storage_tests.rs"]
mod routine_storage_tests;

#[cfg(test)]
#[path = "routine_storage_migration_tests.rs"]
mod routine_storage_migration_tests;

#[cfg(test)]
#[path = "routine_storage_snooze_tests.rs"]
mod routine_storage_snooze_tests;
