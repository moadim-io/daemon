//! One-time startup migrations for the on-disk routine layout: legacy `prompt.txt`/`prompt.md`
//! renames, the prompt-subfolder restructuring, UUID-to-slug directory renames, and the
//! TOML-sidecar-to-append-only-log migration for trigger timestamps.

use super::{
    load_routine_from_dir, read_routine_toml, remove_routine_dir, routines_dir, slugify,
    write_routine, RuntimeState,
};
use serde::{Deserialize, Serialize};

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
        if let Err(err) = crate::utils::fs_perms::create_private_dir_all(&prompts_dir) {
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
