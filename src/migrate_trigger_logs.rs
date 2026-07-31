#![allow(
    clippy::wildcard_imports,
    clippy::too_many_arguments,
    clippy::needless_pass_by_value,
    dead_code,
    reason = "split-out module preserves existing code while keeping files under the linecheck limit"
)]
use super::*;

/// Migrate per-routine trigger state from legacy TOML sidecars to append-only log files.
///
/// For each routine directory:
/// - If `scheduled.local.toml` exists and `scheduled.log` does not, the stored timestamp is
///   written as the first log line and the TOML file is removed.
/// - If `state.local.toml` contains a `last_manual_trigger_at` field and `manual.log` does not
///   exist, the stored timestamp is written as the first log line.
///
/// Call once at startup, after [`migrate_prompt_files`] and before [`crate::routine_storage::load_store`].
pub fn migrate_trigger_logs() {
    migrate_trigger_logs_from_dir(&routines_dir());
}

/// Inner variant of [`migrate_trigger_logs()`] that scans `dir` instead of [`routines_dir`].
pub(crate) fn migrate_trigger_logs_from_dir(dir: &std::path::Path) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        if !entry.file_type().is_ok_and(|ft| ft.is_dir()) {
            continue;
        }
        let routine_dir = entry.path();

        // Migrate scheduled.local.toml → scheduled.log
        let old_sched = routine_dir.join("scheduled.local.toml");
        let new_sched = routine_dir.join("scheduled.log");
        if old_sched.exists() && !new_sched.exists() {
            if let Some(ts) = std::fs::read_to_string(&old_sched)
                .ok()
                .and_then(|text| toml::from_str::<LegacyScheduledState>(&text).ok())
                .and_then(|state| state.last_scheduled_trigger_at)
            {
                let line = format!("{ts}\n");
                if let Err(err) = std::fs::write(&new_sched, line.as_bytes()) {
                    log::warn!(
                        "migrate_trigger_logs: failed to write {}: {err}",
                        new_sched.display()
                    );
                    continue;
                }
            }
            let _ = std::fs::remove_file(&old_sched);
        }

        // Migrate last_manual_trigger_at from state.local.toml → manual.log
        let new_manual = routine_dir.join("manual.log");
        if !new_manual.exists() {
            if let Some(ts) = std::fs::read_to_string(routine_dir.join("state.local.toml"))
                .ok()
                .and_then(|text| toml::from_str::<RuntimeState>(&text).ok())
                .and_then(|state| state.last_manual_trigger_at)
            {
                let line = format!("{ts}\n");
                if let Err(err) = std::fs::write(&new_manual, line.as_bytes()) {
                    log::warn!(
                        "migrate_trigger_logs: failed to write {}: {err}",
                        new_manual.display()
                    );
                }
            }
        }
    }
}
