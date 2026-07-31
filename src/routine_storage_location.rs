//! Filesystem-derived routine location helpers.
//!
//! Routine organization is owned by the directory tree under `routines/`, not by duplicated TOML
//! metadata. These helpers find a routine's current relative directory by reading `routine.toml`
//! ids and fall back to the title slug only for new routines that have never been persisted.

use serde::Deserialize;

use crate::paths::routines_dir;
use crate::routines::{slugify, Routine};

#[derive(Deserialize)]
/// Minimal TOML view used to match a persisted routine directory by stable id.
struct IdOnlyRoutineToml {
    /// Stable routine id from `routine.toml`.
    id: Option<String>,
}

/// Convert a relative path to the API's slash-separated representation.
fn rel_string(path: &std::path::Path) -> String {
    path.components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

/// Read only the stable routine id from a `routine.toml` path.
fn read_id(path: &std::path::Path) -> Option<String> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|text| toml::from_str::<IdOnlyRoutineToml>(&text).ok())
        .and_then(|routine| routine.id)
}

/// Recursively find the directory whose `routine.toml` contains `id`.
fn find_rel_dir_by_id(base: &std::path::Path, dir: &std::path::Path, id: &str) -> Option<String> {
    let entries = std::fs::read_dir(dir).ok()?;
    for entry in entries.flatten() {
        if !entry.file_type().is_ok_and(|ft| ft.is_dir()) {
            continue;
        }
        let path = entry.path();
        let toml = path.join("routine.toml");
        if toml.exists() && read_id(&toml).as_deref() == Some(id) {
            return path.strip_prefix(base).ok().map(rel_string);
        }
        if let Some(found) = find_rel_dir_by_id(base, &path, id) {
            return Some(found);
        }
    }
    None
}

/// Return the current routine directory relative to `routines/`, if a persisted routine with `id`
/// exists anywhere under the recursive routines tree.
pub(crate) fn routine_rel_dir_by_id(id: &str) -> Option<String> {
    let base = routines_dir();
    find_rel_dir_by_id(&base, &base, id)
}

/// Return the relative directory a routine should use for filesystem sidecars.
///
/// Existing routines keep their current disk location. New routines fall back to the slugified title.
pub(crate) fn routine_rel_dir(routine: &Routine) -> String {
    routine_rel_dir_by_id(&routine.id).unwrap_or_else(|| slugify(&routine.title))
}

/// Return the last path segment of a routine's filesystem location.
pub(crate) fn routine_slug(routine: &Routine) -> String {
    slug_from_rel_dir(&routine_rel_dir(routine))
}

/// Return the last path segment of an already-computed [`routine_rel_dir`] value.
///
/// Pure string derivation — no filesystem access. Prefer this over [`routine_slug`] when a
/// `rel_dir` has already been computed for the same routine, so callers that need both the
/// directory and its slug (or folder) don't each trigger their own recursive tree walk.
pub(crate) fn slug_from_rel_dir(rel_dir: &str) -> String {
    rel_dir.rsplit('/').next().unwrap_or("").to_string()
}

/// Return the parent folder of an already-computed [`routine_rel_dir`] value, relative to
/// `routines/`.
///
/// Pure string derivation — no filesystem access. See [`slug_from_rel_dir`] for why this exists
/// as a companion for callers that already computed a relative directory.
pub(crate) fn folder_from_rel_dir(rel_dir: &str) -> Option<String> {
    std::path::Path::new(rel_dir)
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .map(rel_string)
}
