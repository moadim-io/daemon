//! TOML-backed persistence for routines, plus the composed `prompt.txt` sidecar file.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};

use crate::paths::{
    routine_dir, routine_gitignore_path, routine_prompt_path, routine_toml_path, routines_dir,
};
use crate::routines::{compose_prompt, Repository, Routine, RoutineStore};

/// TOML representation of a routine on disk.
#[derive(Debug, Deserialize, Serialize)]
struct RoutineToml {
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

/// Load a routine from `{routines_dir}/{id}/routine.toml`.
fn load_routine_from_dir(id: &str) -> Option<Routine> {
    let t = read_routine_toml(&routine_toml_path(id))?;
    Some(Routine {
        id: id.to_string(),
        schedule: t.schedule?,
        title: t.title?,
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

/// Write `routine` to disk: `routine.toml`, the composed `prompt.txt`, and `.gitignore` if absent.
pub fn write_routine(routine: &Routine) -> std::io::Result<()> {
    let dir = routine_dir(&routine.id);
    std::fs::create_dir_all(&dir)?;

    let gitignore = routine_gitignore_path(&routine.id);
    if !gitignore.exists() {
        std::fs::write(&gitignore, "*.local.*\n*.log\n")?;
    }

    let toml_routine = RoutineToml {
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
    std::fs::write(routine_toml_path(&routine.id), text)?;
    std::fs::write(routine_prompt_path(&routine.id), compose_prompt(routine))?;
    Ok(())
}

/// Remove the directory for routine `id`, doing nothing if it does not exist.
pub fn remove_routine_dir(id: &str) -> std::io::Result<()> {
    let dir = routine_dir(id);
    if dir.exists() {
        std::fs::remove_dir_all(dir)?;
    }
    Ok(())
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
                let id = entry.file_name().to_string_lossy().to_string();
                if let Some(routine) = load_routine_from_dir(&id) {
                    routines.insert(id, routine);
                }
            }
        }
    }
    Arc::new(Mutex::new(routines))
}

#[cfg(test)]
#[path = "routine_storage_tests.rs"]
mod routine_storage_tests;
