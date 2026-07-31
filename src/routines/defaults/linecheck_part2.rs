#![allow(
    clippy::wildcard_imports,
    clippy::too_many_arguments,
    clippy::needless_pass_by_value,
    dead_code,
    reason = "split-out module preserves existing code while keeping files under the linecheck limit"
)]
use super::*;

/// Persist `slugs` to the tombstone file, creating its parent directory if needed.
pub(crate) fn write_removed_defaults(slugs: &BTreeSet<String>) -> std::io::Result<()> {
    let path = removed_default_routines_path();
    let parent = crate::utils::fs_perms::parent_or_err(&path, "removed-defaults tombstone")?;
    std::fs::create_dir_all(parent)?;
    let body = RemovedDefaults {
        slugs: slugs.clone(),
    };
    let toml = toml::to_string_pretty(&body).map_err(std::io::Error::other)?;
    std::fs::write(path, toml)
}

/// Process-wide lock serializing the tombstone-file read-modify-write sequence.
///
/// [`record_removed_default`] and [`clear_removed_default`] each read the whole file, mutate the
/// slug set, and write it back in full — `DELETE /routines/{id}` and `POST /routines` (which call
/// them from `svc_delete`/`svc_create` respectively) can run concurrently on the multi-thread
/// runtime, so two overlapping round trips can interleave and the later write wins outright,
/// silently dropping whichever change the other request had just persisted (e.g. deleting two
/// different default routines back to back could lose one tombstone, resurrecting a routine the
/// user explicitly removed on the next startup). Same hazard class as the crontab read-modify-write
/// race (issue #365, see `crontab_sync_lock` in `src/sync/routines.rs`) and the `machine.local.toml`
/// race (`machine_toml_lock` in `src/machine/mod.rs`), fixed the same way: hold this lock across
/// the whole read-then-write span.
pub(crate) fn removed_defaults_lock() -> &'static Mutex<()> {
    pub(crate) static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

/// Record that the built-in default identified by `slug` was explicitly deleted, so
/// [`ensure_default_routines`] does not re-create it on the next startup. Best-effort: a write
/// failure is logged rather than propagated, matching [`ensure_default_routines`]'s own
/// best-effort persistence.
pub fn record_removed_default(slug: &str) {
    let _guard = removed_defaults_lock().lock_recover();
    let mut slugs = read_removed_defaults();
    if slugs.insert(slug.to_string()) {
        if let Err(err) = write_removed_defaults(&slugs) {
            log::warn!("record_removed_default: failed to persist tombstone for {slug:?}: {err}");
        }
    }
}

/// Clear the tombstone for `slug`, if any, letting [`ensure_default_routines`] re-seed it on the
/// next startup. Called when the user creates a routine whose title matches a tombstoned default.
pub fn clear_removed_default(slug: &str) {
    let _guard = removed_defaults_lock().lock_recover();
    let mut slugs = read_removed_defaults();
    if slugs.remove(slug) {
        if let Err(err) = write_removed_defaults(&slugs) {
            log::warn!("clear_removed_default: failed to clear tombstone for {slug:?}: {err}");
        }
    }
}
