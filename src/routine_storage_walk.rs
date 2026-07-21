//! Recursive walk helper for loading routines from nested folders.

use std::collections::HashMap;

use crate::routines::Routine;

/// Walk `dir` recursively, loading any child folder that contains a `routine.toml`.
///
/// `base` stays fixed at the scan root so nested routine folders load from a coherent relative
/// path. `load` is injected so the recursive walk stays testable without tying the helper to the
/// surrounding module's private loader.
pub(super) fn walk_routines(
    base: &std::path::Path,
    dir: &std::path::Path,
    routines: &mut HashMap<String, Routine>,
    load: &dyn Fn(&std::path::Path, &str) -> Option<Routine>,
) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            if !entry.file_type().is_ok_and(|ft| ft.is_dir()) {
                continue;
            }
            let path = entry.path();
            if path.join("routine.toml").exists() {
                let rel = path.strip_prefix(base).unwrap_or(&path);
                let rel = rel.to_string_lossy().to_string();
                match load(base, &rel) {
                    Some(routine) => {
                        routines.insert(routine.id.clone(), routine);
                    }
                    None => {
                        log::warn!(
                            "load_store: skipping routine dir {rel:?}: its routine.toml is \
                             unparsable or missing a required field (title, schedule, or agent)"
                        );
                    }
                }
            } else {
                walk_routines(base, &path, routines, load);
            }
        }
    }
}

// Tests must live in a `*_tests.rs` sibling per this repo's convention (see
// CONTRIBUTING.md and `.githooks/pre-push`'s test-file-convention check), not an inline
// `#[cfg(test)] mod tests { ... }` block.
#[cfg(test)]
#[path = "routine_storage_walk_tests.rs"]
mod routine_storage_walk_tests;
