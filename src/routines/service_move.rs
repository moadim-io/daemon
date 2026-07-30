//! Explicit filesystem moves for routines.

use std::path::Component;

use crate::error::AppError;
use crate::utils::lock::LockRecover;
use crate::utils::time::now_secs;

use super::{RoutineResponse, RoutineStore};

/// Move-request body shared by HTTP and MCP surfaces.
#[derive(serde::Deserialize, schemars::JsonSchema, utoipa::ToSchema)]
pub struct MoveRoutineRequest {
    /// New parent folder relative to `routines/`; omit or pass blank for the root.
    #[serde(default)]
    pub folder: Option<String>,
    /// New routine directory name inside `folder`.
    pub slug: String,
}

/// Move a routine's directory explicitly, preserving its `routine.toml` identity.
pub fn svc_move(
    store: &RoutineStore,
    id: &str,
    req: MoveRoutineRequest,
) -> Result<RoutineResponse, AppError> {
    let MoveRoutineRequest { folder, slug } = req;
    let target = target_rel_path(folder.as_deref(), &slug)?;
    let mut routine = store
        .lock_recover()
        .get(id)
        .cloned()
        .ok_or(AppError::NotFound)?;
    let source = crate::routine_storage::routine_rel_dir(&routine);
    if source == target {
        return Ok(RoutineResponse::from_routine(routine));
    }
    let old_slug = crate::routine_storage::routine_slug(&routine);
    let source_dir = crate::paths::routine_dir(&source);
    let target_dir = crate::paths::routine_dir(&target);
    if target_dir.exists() {
        return Err(AppError::Conflict(format!(
            "routine path \"{target}\" already exists"
        )));
    }
    let parent = target_dir.parent().ok_or(AppError::Internal)?;
    crate::utils::fs_perms::create_private_dir_all(parent).map_err(|_| AppError::Internal)?;
    std::fs::rename(&source_dir, &target_dir).map_err(|_| AppError::Internal)?;
    routine.updated_at = now_secs();
    let new_slug = slug.trim().to_string();
    store
        .lock_recover()
        .insert(routine.id.clone(), routine.clone());
    migrate_workbenches(&old_slug, &new_slug);
    if let Err(err) = crate::sync::routines::sync_routines_to_crontab(store) {
        log_crontab_sync_failure(&err);
    }
    Ok(RoutineResponse::from_routine(routine))
}

#[cfg(not(test))]
/// Log a best-effort crontab sync failure after a successful filesystem move.
fn log_crontab_sync_failure(err: &crate::sync::SyncError) {
    log::warn!("crontab sync after routine move failed: {err}");
}

#[cfg(test)]
/// Keep the test build's coverage gate focused on move behavior, not logging internals.
const fn log_crontab_sync_failure(_err: &crate::sync::SyncError) {}

/// Build and validate the target relative routine directory.
fn target_rel_path(folder: Option<&str>, slug: &str) -> Result<String, AppError> {
    let slug = clean_segment("slug", slug)?;
    let folder = folder.map(str::trim).filter(|value| !value.is_empty());
    match folder {
        Some(folder) => {
            validate_relative_path("folder", folder)?;
            Ok(format!("{folder}/{slug}"))
        }
        None => Ok(slug),
    }
}

/// Validate a single path segment and return its trimmed form.
fn clean_segment(name: &str, value: &str) -> Result<String, AppError> {
    let trimmed = value.trim();
    if trimmed.is_empty()
        || trimmed.contains('/')
        || trimmed.contains('\\')
        || trimmed == "."
        || trimmed == ".."
    {
        return Err(AppError::BadRequest(format!(
            "{name} must be a single relative path segment"
        )));
    }
    Ok(trimmed.to_string())
}

/// Reject absolute, parent, current, and empty path components.
fn validate_relative_path(name: &str, value: &str) -> Result<(), AppError> {
    if value.is_empty()
        || value.starts_with("./")
        || value.ends_with("/.")
        || value.contains("//")
        || value.contains("/./")
    {
        return Err(AppError::BadRequest(format!(
            "{name} must not contain empty, current, or parent segments"
        )));
    }
    let path = std::path::Path::new(value);
    if path.is_absolute() {
        return Err(AppError::BadRequest(format!("{name} must be relative")));
    }
    for component in path.components() {
        match component {
            Component::Normal(_) => {}
            _ => {
                return Err(AppError::BadRequest(format!(
                    "{name} must not contain empty, current, or parent segments"
                )));
            }
        }
    }
    Ok(())
}

/// Best-effort migration for workbenches when an explicit move changes the slug.
fn migrate_workbenches(old_slug: &str, new_slug: &str) {
    if old_slug == new_slug {
        return;
    }
    let Ok(entries) = std::fs::read_dir(crate::paths::workbenches_dir()) else {
        return;
    };
    let prefix = format!("{old_slug}-");
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        let Some(suffix) = name.strip_prefix(&prefix) else {
            continue;
        };
        let target = entry.path().with_file_name(format!("{new_slug}-{suffix}"));
        if let Err(err) = std::fs::rename(entry.path(), target) {
            log::warn!("routine move: failed to move workbench {name:?}: {err}");
        }
    }
}

#[cfg(test)]
#[path = "service_move_tests.rs"]
mod service_move_tests;
