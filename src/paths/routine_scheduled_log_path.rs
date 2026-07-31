
/// Returns the path to `{routines_dir}/{id}/scheduled.log`, the gitignored append-only log that
/// records every scheduled (cron) firing as one Unix-timestamp line.
///
/// The cron shell command appends a line (`printf '%s\n' "$TS" >> scheduled.log`) at each firing;
/// the daemon reads only the last line to derive `last_scheduled_trigger_at`. The `.log` suffix
/// matches the `*.log` pattern seeded into the config `.gitignore`.
#[must_use]
pub fn routine_scheduled_log_path(id: &str) -> PathBuf {
    routine_dir(id).join("scheduled.log")
}

/// Returns the path to `{routines_dir}/{id}/manual.log`, the gitignored append-only log that
/// records every manual trigger as one Unix-timestamp line.
///
/// The daemon appends a line at each manual trigger; reading the last line gives
/// `last_manual_trigger_at`. The `.log` suffix matches the `*.log` pattern in the config
/// `.gitignore`.
#[must_use]
pub fn routine_manual_log_path(id: &str) -> PathBuf {
    routine_dir(id).join("manual.log")
}

/// Returns the path to `{routines_dir}/{id}/skip.log`, the gitignored append-only log recording
/// why a trigger did not spawn a workbench (agent load failure, an oversized inline prompt, the
/// per-routine overlap guard, or the global concurrency cap — see
/// `crate::routines::service_trigger::spawn_routine_command`).
///
/// Without this, a skipped trigger left no trace anywhere a caller could read back: `routine_logs`
/// looks up the newest *workbench's* `agent.log`, and a skipped trigger never creates a workbench
/// (#1145). The `.log` suffix matches the `*.log` pattern in the config `.gitignore`.
#[must_use]
pub fn routine_skip_log_path(id: &str) -> PathBuf {
    routine_dir(id).join("skip.log")
}

/// Returns the path to `{routines_dir}/{id}/runs.log`, the gitignored append-only NDJSON log of
/// every finished run's outcome, keyed by the routine's stable UUID (unlike its workbenches, which
/// are keyed by slug and reaped after their TTL).
///
/// One compact JSON object is appended per run, right before its workbench is reaped (see
/// `routines::cleanup::reap_dir`), so run history survives past workbench retention instead of
/// disappearing the moment its workbench directory is removed. The `.log` suffix matches the
/// `*.log` pattern seeded into the config `.gitignore`.
#[must_use]
pub fn routine_run_history_path(id: &str) -> PathBuf {
    routine_dir(id).join("runs.log")
}

/// Returns the path to `{config_dir}/removed_defaults.local.toml`, the gitignored file recording
/// which built-in default routines the user has explicitly deleted, so
/// [`crate::routines::ensure_default_routines`] does not resurrect them on the next startup. The
/// `.local.` infix matches the `*.local.*` pattern seeded into the config `.gitignore`.
#[must_use]
pub fn removed_default_routines_path() -> PathBuf {
    config_dir().join("removed_defaults.local.toml")
}

/// Returns the path to `{routines_dir}/{id}/run.sh`, a legacy per-routine launch script.
///
/// No longer generated — the crontab line now invokes `moadim schedule trigger <id>` directly. The
/// path is retained so [`crate::routine_storage::write_routine`] can delete any stale script left by
/// an older daemon.
#[must_use]
pub fn routine_script_path(id: &str) -> PathBuf {
    routine_dir(id).join("run.sh")
}

/// Returns the path to `{routines_dir}/{id}/flags/`, holding one file per open flag an agent (or a
/// human, via MCP/HTTP) has raised against the routine — a gap, bug, edge case, or question it
/// couldn't resolve mid-run. See [`crate::routines::flags`].
#[must_use]
pub fn routine_flags_dir(id: &str) -> PathBuf {
    routine_dir(id).join("flags")
}

// ─── Agent registry ──────────────────────────────────────────────────────────

/// Returns the path to `{config_dir}/agents/` (default `~/.config/moadim/agents/`).
#[must_use]
pub fn agents_dir() -> PathBuf {
    config_dir().join("agents")
}

#[cfg(test)]
mod mod_tests;
