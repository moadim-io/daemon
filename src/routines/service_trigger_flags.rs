//! Flag CRUD for routines: raise, list, and resolve flags raised against a routine.

use crate::error::AppError;
use crate::routine_storage::write_routine;
use crate::routines::command::slugify;
use crate::routines::flags::{self, Flag, FlagScope};
use crate::routines::model::{Routine, RoutineStore};
use crate::utils::lock::LockRecover;

/// Reject a blank (empty/whitespace-only) flag `type` or `description`.
fn validate_flag_field(field: &str, value: &str) -> Result<(), AppError> {
    if value.trim().is_empty() {
        return Err(AppError::BadRequest(format!(
            "flag {field} must not be empty"
        )));
    }
    Ok(())
}

/// Parse a `scope` string into a [`FlagScope`], returning `400 BadRequest` on unknown values.
/// Mirrors `parse_lock_scope` in `handlers.rs`.
fn parse_flag_scope(scope: &str) -> Result<FlagScope, AppError> {
    match scope {
        "general" => Ok(FlagScope::General),
        "local" => Ok(FlagScope::Local),
        other => Err(AppError::BadRequest(format!(
            "unknown flag scope {other:?}; use \"general\" or \"local\""
        ))),
    }
}

/// Look up a routine by `id` and derive its slug, `NotFound` if it does not exist.
fn routine_and_slug(store: &RoutineStore, id: &str) -> Result<(Routine, String), AppError> {
    let routine = store
        .lock_recover()
        .get(id)
        .cloned()
        .ok_or(AppError::NotFound)?;
    let slug = slugify(&routine.title);
    Ok((routine, slug))
}

/// Raise a new flag against routine `id`. `flag_type` and `description` must be non-blank;
/// `scope` is `"general"` (committed) or `"local"` (gitignored). Refreshes the routine's
/// `prompts/prompt.compiled.local.md` afterward so the next run's "Open flags" section (see
/// `compose_prompt`) includes it.
pub fn svc_create_flag(
    store: &RoutineStore,
    id: &str,
    flag_type: &str,
    description: &str,
    scope: &str,
) -> Result<Flag, AppError> {
    validate_flag_field("type", flag_type)?;
    validate_flag_field("description", description)?;
    let scope = parse_flag_scope(scope)?;
    let (routine, slug) = routine_and_slug(store, id)?;
    let flag =
        flags::create_flag(&slug, flag_type, description, scope).map_err(|_| AppError::Internal)?;
    write_routine(&routine).map_err(|_| AppError::Internal)?;
    Ok(flag)
}

/// List every open flag raised against routine `id`, oldest first.
pub fn svc_list_flags(store: &RoutineStore, id: &str) -> Result<Vec<Flag>, AppError> {
    let (_, slug) = routine_and_slug(store, id)?;
    Ok(flags::list_flags(&slug))
}

/// Resolve (delete) the flag named `filename` under routine `id`.
///
/// `NotFound` when the routine does not exist, `filename` is unsafe, or names no existing flag.
/// Refreshes `prompts/prompt.compiled.local.md` afterward so a resolved flag stops appearing in the next
/// run's prompt.
pub fn svc_resolve_flag(store: &RoutineStore, id: &str, filename: &str) -> Result<(), AppError> {
    let (routine, slug) = routine_and_slug(store, id)?;
    let resolved = flags::resolve_flag(&slug, filename).map_err(|_| AppError::Internal)?;
    if !resolved {
        return Err(AppError::NotFound);
    }
    write_routine(&routine).map_err(|_| AppError::Internal)?;
    Ok(())
}
