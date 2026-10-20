
/// Append a Unix-timestamp entry to a routine's `manual.log`, recording a manual trigger.
///
/// Called by `svc_trigger` immediately after stamping `last_manual_trigger_at` on the in-memory
/// routine. Best-effort: a log-write failure is warned but never surfaced to the caller, so a
/// disk hiccup can't block the trigger itself.
pub fn append_manual_trigger_log(slug: &str, ts: u64) {
    let path = routine_manual_log_path(slug);
    let line = format!("{ts}\n");
    if let Err(err) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .and_then(|mut file| std::io::Write::write_all(&mut file, line.as_bytes()))
    {
        log::warn!(
            "append_manual_trigger_log: failed to write {}: {err}",
            path.display()
        );
    }
}

/// Append a Unix-timestamp entry to a routine's `scheduled.log`, recording an accepted scheduled
/// fire before its launcher process is attempted.
///
/// The in-process scheduler cannot rely on the detached launcher to leave this evidence: an agent
/// setup or shell failure would otherwise make a real scheduler fire indistinguishable from a
/// missed tick. Best-effort like [`append_manual_trigger_log`], so a disk hiccup never blocks the
/// scheduler from attempting the routine.
pub fn append_scheduled_trigger_log(slug: &str, ts: u64) {
    let path = crate::paths::routine_scheduled_log_path(slug);
    let line = format!("{ts}\n");
    if let Err(err) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .and_then(|mut file| std::io::Write::write_all(&mut file, line.as_bytes()))
    {
        log::warn!(
            "append_scheduled_trigger_log: failed to write {}: {err}",
            path.display()
        );
    }
}

/// Append a `{ts}\t{reason}` entry to a routine's `skip.log`, recording why a trigger did not
/// spawn a workbench.
///
/// Called by `spawn_routine_command` from every branch that returns without launching (agent load
/// failure, an oversized inline prompt, the overlap guard, or the global concurrency cap), so
/// `routine_logs` has something to show instead of coming back empty when the newest — or only —
/// signal for a skipped trigger previously lived solely in the daemon's own process log (#1145).
/// Best-effort, like [`append_manual_trigger_log`]: a log-write failure is warned but never
/// surfaced, so a disk hiccup can't turn a skip into a harder failure.
pub fn append_skip_log(slug: &str, ts: u64, reason: &str) {
    let path = routine_skip_log_path(slug);
    let line = format!("{ts}\t{reason}\n");
    if let Err(err) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .and_then(|mut file| std::io::Write::write_all(&mut file, line.as_bytes()))
    {
        log::warn!("append_skip_log: failed to write {}: {err}", path.display());
    }
}

/// Remove the directory for a routine identified by its slug, doing nothing if it does not exist.
pub fn remove_routine_dir(slug: &str) -> std::io::Result<()> {
    let dir = routine_dir(slug);
    if dir.exists() {
        std::fs::remove_dir_all(dir)?;
    }
    Ok(())
}

/// Re-persist every loaded routine to disk, recreating `routine.toml`, `schedule.cron`,
/// `prompts/prompt.pure.md`, and `prompts/prompt.compiled.local.md` in its canonical
/// slug directory.
///
/// Nothing else rewrites the prompt sidecars on startup, so a slug dir missing its
/// `prompts/prompt.compiled.local.md` (e.g. after the UUID→slug migration, or if the sidecar was
/// lost) would fail the launch command's `cp prompt.compiled.local.md`. Re-persisting from the
/// in-memory store
/// heals those dirs (and removes any stale legacy `run.sh`). Idempotent; safe to call on every
/// startup after [`load_store`].
pub fn repersist_routines(store: &RoutineStore) {
    let routines: Vec<Routine> = store.lock_recover().values().cloned().collect();
    for routine in &routines {
        if let Err(err) = write_routine(routine) {
            log::warn!(
                "repersist_routines: failed to write routine {:?}: {err}",
                routine.id
            );
        }
    }
}
