
/// Migrate legacy UUID-named routine directories to the current slug-based layout.
///
/// Early daemon versions stored each routine under `{routines_dir}/{id}/` (the UUID). The current
/// layout uses `{routines_dir}/{slugify(title)}/`. After an upgrade the legacy dir still holds the
/// real `routine.toml` + `prompts/prompt.compiled.local.md`, while the crontab sync creates a *fresh*
/// slug dir containing only `run.sh` — so the cron `cp prompt.compiled.local.md` reads an empty dir
/// and the agent launches task-less.
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
        seed_schedule_cron_from_legacy_toml(&entry.path());
        let Some(routine) = load_routine_from_dir(&dir_name) else {
            // A dir without a parsable routine.toml (e.g. a sync-created dir holding only run.sh)
            // carries no routine to migrate; the routine it shadows is healed from its own dir.
            continue;
        };
        let slug = slugify(&routine.title);
        if slug == dir_name {
            continue;
        }
        if let Err(err) = write_routine_to_rel_dir(&routine, &slug) {
            log::warn!("migrate_routine_dirs: failed to write {slug:?}: {err}; leaving legacy dir");
            continue;
        }
        if let Err(err) = remove_routine_dir(&dir_name) {
            log::warn!("migrate_routine_dirs: failed to remove legacy dir {dir_name:?}: {err}");
        }
    }
}

/// Rename each routine's compiled-prompt sidecar from the legacy `prompt.compiled.md` to
/// `prompt.compiled.local.md`, so it matches the `*.local.*` `.gitignore` pattern instead of relying
/// on the (now removed) explicit `prompt.compiled.md` entry.
///
/// Call once at startup, after [`migrate_prompts_to_subfolder`] (which moves the sidecar into
/// `prompts/`) and before `load_store`. This only renames the file on disk; it does not touch git
/// history or the index — the daemon has no git integration, so an install where
/// `prompt.compiled.md` was already `git add`-ed/committed before this rename must `git rm --cached`
/// it manually (or let the next commit record the rename itself) (issue #1046).
pub fn migrate_compiled_prompt_filename() {
    migrate_compiled_prompt_filename_from_dir(&routines_dir());
}

/// Inner variant of [`migrate_compiled_prompt_filename`] that scans `dir` instead of [`routines_dir`].
///
/// Extracted so tests can drive the migration against a controlled scratch directory, including the
/// `read_dir` error-return branch and the per-entry rename-failure branch.
pub(crate) fn migrate_compiled_prompt_filename_from_dir(dir: &std::path::Path) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        if !entry.file_type().is_ok_and(|ft| ft.is_dir()) {
            continue;
        }
        let prompts_dir = entry.path().join("prompts");
        let old = prompts_dir.join("prompt.compiled.md");
        let new = prompts_dir.join("prompt.compiled.local.md");
        if old.exists() && !new.exists() {
            if let Err(err) = std::fs::rename(&old, &new) {
                log::warn!(
                    "migrate_compiled_prompt_filename: failed to rename {}: {err}",
                    old.display()
                );
            }
        }
    }
}
