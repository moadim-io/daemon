use super::{routines_dir, slugify};

/// Move flags stranded under a routine's title-slug directory into its real on-disk directory.
///
/// Before #1514, flag operations keyed on `slugify(title)`, so a routine living in a folder, or
/// renamed since, had its flags written to a phantom `routines/<title-slug>/flags/` that nothing
/// reads now that flags key on [`crate::routine_storage::routine_rel_dir`].
///
/// Call once at startup, right after [`crate::routine_storage::load_store`].
pub fn migrate_phantom_flags(store: &crate::routines::RoutineStore) {
    use crate::utils::lock::LockRecover;
    let base = routines_dir();
    #[allow(
        clippy::iter_over_hash_type,
        reason = "each routine's flags move independently, so iteration order cannot matter"
    )]
    for routine in store.lock_recover().values() {
        let rel_dir = crate::routine_storage::routine_rel_dir(routine);
        move_phantom_flags(&base, &slugify(&routine.title), &rel_dir);
    }
}

/// Inner variant of [`migrate_phantom_flags`]: move `base/<slug>/flags/*` into
/// `base/<rel_dir>/flags/`.
///
/// No-op when the two directories are the same, or when `base/<slug>` holds a `routine.toml`
/// (it is then another routine's real directory, not a phantom). A flag whose name already exists
/// at the destination is left in place; the phantom directories are removed only once empty.
pub(crate) fn move_phantom_flags(base: &std::path::Path, slug: &str, rel_dir: &str) {
    if slug == rel_dir {
        return;
    }
    let phantom = base.join(slug);
    if phantom.join("routine.toml").exists() {
        return;
    }
    let Ok(entries) = std::fs::read_dir(phantom.join("flags")) else {
        return;
    };
    let dest = base.join(rel_dir).join("flags");
    if std::fs::create_dir_all(&dest).is_err() {
        return;
    }
    for entry in entries.flatten() {
        let target = dest.join(entry.file_name());
        // Never overwrite: a colliding or unmovable flag simply stays in the phantom directory.
        if !target.exists() {
            let _ = std::fs::rename(entry.path(), &target);
        }
    }
    // `remove_dir` only removes empty directories, so a skipped flag keeps its phantom dir.
    let _ = std::fs::remove_dir(phantom.join("flags"));
    let _ = std::fs::remove_dir(&phantom);
}

#[cfg(test)]
#[path = "routine_storage_phantom_flags_tests.rs"]
mod routine_storage_phantom_flags_tests;
