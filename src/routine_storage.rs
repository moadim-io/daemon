//! TOML-backed persistence for routines, plus the composed `prompt.md` sidecar file.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};

use crate::paths::{
    routine_dir, routine_gitignore_path, routine_prompt_path, routine_toml_path, routines_dir,
};
use crate::routines::{compose_prompt, slugify, Repository, Routine, RoutineStore};

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
    last_triggered_at: Option<u64>,
}

/// Parse a routine TOML file at `path`, returning `None` on any error.
fn read_routine_toml(path: &std::path::PathBuf) -> Option<RoutineToml> {
    let text = std::fs::read_to_string(path).ok()?;
    toml::from_str(&text).ok()
}

/// Load a routine from `{routines_dir}/{dir_name}/routine.toml`.
///
/// `dir_name` is the slug (title-derived folder name). The routine's UUID `id` is read from
/// `routine.toml`; for legacy dirs created before this change `id` falls back to `dir_name`.
fn load_routine_from_dir(dir_name: &str) -> Option<Routine> {
    let t = read_routine_toml(&routine_toml_path(dir_name))?;
    let title = t.title?;
    let id = t.id.unwrap_or_else(|| dir_name.to_string());
    Some(Routine {
        id,
        schedule: t.schedule?,
        title,
        agent: t.agent?,
        prompt: t.prompt.unwrap_or_default(),
        repositories: t.repositories,
        enabled: t.enabled.unwrap_or(true),
        source: "managed".to_string(),
        created_at: t.created_at.unwrap_or(0),
        updated_at: t.updated_at.unwrap_or(0),
        last_triggered_at: t.last_triggered_at,
    })
}

/// Write `routine` to disk: `routine.toml`, the composed `prompt.md`, and `.gitignore` if absent.
///
/// The folder is named after the slugified title (`slugify(&routine.title)`). The UUID `id` is
/// stored inside `routine.toml` so it survives a rename.
pub fn write_routine(routine: &Routine) -> std::io::Result<()> {
    let slug = slugify(&routine.title);
    let dir = routine_dir(&slug);
    std::fs::create_dir_all(&dir)?;

    let gitignore = routine_gitignore_path(&slug);
    if !gitignore.exists() {
        std::fs::write(&gitignore, "*.local.*\n*.log\n")?;
    }

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
        last_triggered_at: routine.last_triggered_at,
    };
    let text = toml::to_string_pretty(&toml_routine).map_err(std::io::Error::other)?;
    std::fs::write(routine_toml_path(&slug), text)?;
    std::fs::write(routine_prompt_path(&slug), compose_prompt(routine))?;
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
    let dir = routines_dir();
    let entries = match std::fs::read_dir(&dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        if !entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            continue;
        }
        let old = entry.path().join("prompt.txt");
        let new = entry.path().join("prompt.md");
        if old.exists() && !new.exists() {
            if let Err(e) = std::fs::rename(&old, &new) {
                log::warn!("migrate_prompt_files: failed to rename {:?}: {e}", old);
            }
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
            if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
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
