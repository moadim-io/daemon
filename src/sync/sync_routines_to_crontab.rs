#![allow(
    clippy::wildcard_imports,
    clippy::too_many_arguments,
    clippy::needless_pass_by_value,
    dead_code,
    reason = "split-out module preserves existing code while keeping files under the linecheck limit"
)]
use super::*;

/// Substring identifying a routine line inside the crontab block (`# moadim-routine:<id>`).
pub(crate) const ROUTINE_LINE_MARKER: &str = "# moadim-routine:";

/// Write all enabled managed routines from `store` into the OS routines crontab block.
///
/// Idempotent: skips the `crontab -` call when the crontab would not change.
///
/// Footgun guard: refuses to overwrite a populated routines block when the store is *empty*. An
/// empty store at sync time means the store never loaded (or a second daemon is racing this one),
/// not a genuine "no routines" state — startup always reseeds the built-in defaults, so the steady
/// state is never an empty store. Without this guard such a sync would write a bare block and
/// silently drop every scheduled routine's cron line (the incident that motivated it). A store that
/// loaded fine but holds only disabled/unmanaged routines is *not* empty, so legitimately clearing
/// the last routine still works.
///
/// Every caller is a REST/MCP async request handler running on the multi-thread runtime
/// (`#[tokio::main]`'s default flavor), but the work below — `crontab -l` / `crontab -` subprocess
/// round trips — is blocking (#360). Run inline, it occupies a worker thread for the whole
/// round-trip; a hung `crontab` binary can tie up enough workers to stall unrelated in-flight
/// requests, including `/health`. [`tokio::task::block_in_place`] tells the runtime this thread is
/// about to block so it can hand its other scheduled tasks to a spare worker. It's only valid (and
/// only needed) on a multi-thread runtime — it panics on `current_thread`, which `#[tokio::test]`
/// defaults to — and only inside a runtime at all (plain `#[test]`s call this function directly
/// with none running), so both are checked first; either falls back to running inline exactly as
/// before.
pub fn sync_routines_to_crontab(store: &RoutineStore) -> Result<(), SyncError> {
    // macOS routines are dispatched by the daemon's in-process scheduler. Do not invoke crontab:
    // its TCC-protected write path can hang the daemon during otherwise ordinary routine changes.
    #[cfg(all(target_os = "macos", not(test)))]
    {
        let _ = store;
        crate::sync::record_crontab_sync_success();
        Ok(())
    }
    #[cfg(not(all(target_os = "macos", not(test))))]
    {
        sync_routines_to_crontab_with_os_crontab(store)
    }
}

/// Synchronize the OS crontab where it is the active routine scheduler.
#[cfg(not(all(target_os = "macos", not(test))))]
fn sync_routines_to_crontab_with_os_crontab(store: &RoutineStore) -> Result<(), SyncError> {
    let on_multi_thread_runtime = tokio::runtime::Handle::try_current()
        .is_ok_and(|handle| handle.runtime_flavor() == tokio::runtime::RuntimeFlavor::MultiThread);
    let result = if on_multi_thread_runtime {
        tokio::task::block_in_place(|| sync_routines_to_crontab_blocking(store))
    } else {
        sync_routines_to_crontab_blocking(store)
    };
    match &result {
        Ok(()) => crate::sync::record_crontab_sync_success(),
        Err(err) => crate::sync::record_crontab_sync_failure(err),
    }
    result
}

/// Blocking body of [`sync_routines_to_crontab()`], split out so the wrapper can choose whether to
/// run it via [`tokio::task::block_in_place`].
pub(crate) fn sync_routines_to_crontab_blocking(store: &RoutineStore) -> Result<(), SyncError> {
    let _crontab_guard = crontab_sync_lock().lock_recover();
    let current = read_crontab()?;
    if store.lock_recover().is_empty() && current.contains(ROUTINE_LINE_MARKER) {
        log::warn!(
            "routine sync: store is empty but the crontab still has routine lines; refusing to \
             wipe the routines block (suspected load failure or a concurrent daemon)"
        );
        return Ok(());
    }
    let block = build_block(store);
    let new_crontab = replace_block_with(&current, &block, BLOCK_BEGIN, BLOCK_END);
    if new_crontab == current {
        return Ok(());
    }
    write_crontab(&new_crontab)
}
