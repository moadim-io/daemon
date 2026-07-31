//! One-time startup migrations for the on-disk routine layout: legacy `prompt.txt`/`prompt.md`
//! renames, the prompt-subfolder restructuring, UUID-to-slug directory renames, and the
//! TOML-sidecar-to-append-only-log migration for trigger timestamps.

use super::{
    load_routine_from_dir, read_routine_toml, remove_routine_dir, routines_dir, slugify,
    write_routine_to_rel_dir, RuntimeState,
};
use serde::{Deserialize, Serialize};

#[allow(
    clippy::missing_docs_in_private_items,
    reason = "split-out module keeps the file under the linecheck limit"
)]
#[path = "routine_storage_migrations_part2.rs"]
mod routine_storage_migrations_part2;
pub(crate) use routine_storage_migrations_part2::*;

/// Minimal view of pre-sidecar `routine.toml` files that still carried the cron schedule.
#[derive(Debug, Deserialize)]
struct LegacyRoutineSchedule {
    /// Legacy cron expression, used only to seed `schedule.cron` during directory migration.
    schedule: Option<String>,
}

/// Seed a missing `schedule.cron` from a legacy `routine.toml` before loading a legacy directory.
pub(crate) fn seed_schedule_cron_from_legacy_toml(routine_dir: &std::path::Path) {
    let cron_path = routine_dir.join("schedule.cron");
    if cron_path.exists() {
        return;
    }
    let Some(schedule) = std::fs::read_to_string(routine_dir.join("routine.toml"))
        .ok()
        .and_then(|text| toml::from_str::<LegacyRoutineSchedule>(&text).ok())
        .and_then(|legacy| legacy.schedule)
    else {
        return;
    };
    if let Err(err) = std::fs::write(&cron_path, format!("{}\n", schedule.trim())) {
        log::warn!(
            "migrate_routine_dirs: failed to seed {} from legacy routine.toml schedule: {err}",
            cron_path.display()
        );
    }
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
/// and before [`migrate_compiled_prompt_filename`] / [`migrate_routine_dirs`] / `load_store`. Older
/// daemons wrote a single top-level `prompt.md` (the composed prompt) and kept the raw prompt inside
/// `routine.toml`'s `prompt` field; this daemon reads the raw prompt from `prompts/prompt.pure.md`
/// and the composed prompt from `prompts/prompt.compiled.local.md`, so an un-migrated dir would
/// launch with an empty prompt. This step lands the composed prompt at the intermediate
/// `prompts/prompt.compiled.md` name; [`migrate_compiled_prompt_filename`] renames it the rest of the
/// way to `prompt.compiled.local.md`.
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
include!("routine_storage_migrations_part3.rs");
