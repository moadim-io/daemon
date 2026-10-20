//! Flag CRUD for routines: raise, list, and resolve flags raised against a routine.

use crate::error::AppError;
use crate::routine_storage::{routine_rel_dir, write_routine};
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
/// Mirrors `parse_scope` in `routes::lock_routines::logic`.
fn parse_flag_scope(scope: &str) -> Result<FlagScope, AppError> {
    match scope {
        "general" => Ok(FlagScope::General),
        "local" => Ok(FlagScope::Local),
        other => Err(AppError::BadRequest(format!(
            "unknown flag scope {other:?}; use \"general\" or \"local\""
        ))),
    }
}

/// Look up a routine by `id` and derive its on-disk relative directory, `NotFound` if it does not
/// exist.
///
/// Flags must be keyed by [`routine_rel_dir`] — the routine's actual filesystem location — rather
/// than `slugify(&routine.title)`: the two diverge for any routine that lives in a folder or has
/// been renamed/moved since its flags directory was first created, silently stranding flags in a
/// phantom directory nothing else reads (see #1514).
fn routine_and_rel_dir(store: &RoutineStore, id: &str) -> Result<(Routine, String), AppError> {
    let routine = store
        .lock_recover()
        .get(id)
        .cloned()
        .ok_or(AppError::NotFound)?;
    let rel_dir = routine_rel_dir(&routine);
    Ok((routine, rel_dir))
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
    let (routine, rel_dir) = routine_and_rel_dir(store, id)?;
    let flag = flags::create_flag(&rel_dir, flag_type, description, scope)
        .map_err(|_| AppError::Internal)?;
    write_routine(&routine).map_err(|_| AppError::Internal)?;
    Ok(flag)
}

/// List every open flag raised against routine `id`, oldest first.
pub fn svc_list_flags(store: &RoutineStore, id: &str) -> Result<Vec<Flag>, AppError> {
    let (_, rel_dir) = routine_and_rel_dir(store, id)?;
    Ok(flags::list_flags(&rel_dir))
}

/// Resolve (delete) the flag named `filename` under routine `id`.
///
/// `NotFound` when the routine does not exist, `filename` is unsafe, or names no existing flag.
/// Refreshes `prompts/prompt.compiled.local.md` afterward so a resolved flag stops appearing in the next
/// run's prompt.
pub fn svc_resolve_flag(store: &RoutineStore, id: &str, filename: &str) -> Result<(), AppError> {
    let (routine, rel_dir) = routine_and_rel_dir(store, id)?;
    let resolved = flags::resolve_flag(&rel_dir, filename).map_err(|_| AppError::Internal)?;
    if !resolved {
        return Err(AppError::NotFound);
    }
    write_routine(&routine).map_err(|_| AppError::Internal)?;
    Ok(())
}
