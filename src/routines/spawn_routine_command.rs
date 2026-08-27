#![allow(
    clippy::wildcard_imports,
    clippy::too_many_arguments,
    clippy::needless_pass_by_value,
    dead_code,
    reason = "split-out module preserves existing code while keeping files under the linecheck limit"
)]
use super::*;

/// Spawn the launch command for `routine` under a login shell, logging (rather than failing) when
/// the agent config cannot be loaded, the composed prompt won't fit in an inlined `{prompt}`
/// argument, a previous fire of this routine is still running, the global concurrency cap is
/// already reached, or the process cannot be spawned.
///
/// `sh -lc` sources the user's `~/.profile`, so the agent inherits their environment (`GH_TOKEN`,
/// API keys, …) regardless of the minimal environment the daemon (or cron) runs under. Shared by the
/// manual ([`svc_trigger`]) and scheduled ([`svc_trigger_scheduled`]) paths. Those services record
/// their durable trigger evidence before reaching this best-effort detached launcher.
pub(crate) fn spawn_routine_command(routine: &Routine, source: TriggerSource) {
    match load_agent_command(&routine.agent) {
        Ok(agent) => {
            // Guard against the silent `execve(E2BIG)` no-op an oversized `{prompt}` argument
            // causes inside the detached tmux session (#443): the OS-level failure never
            // surfaces anywhere, so catch it here instead and skip the launch with a visible
            // warning, the same non-fatal shape as the agent-load-failure arm below.
            if let Some(len) = inline_prompt_overflow(routine, &agent) {
                let reason = format!(
                    "composed prompt is {len} bytes, over the inline-argument limit for agent \
                     {:?}; skipping launch (would fail silently inside tmux otherwise) — switch \
                     the agent's args to {{prompt_file}} or shorten the routine's prompt/open \
                     flags",
                    routine.agent,
                );
                log::warn!("trigger: routine {:?} skipped — {reason}", routine.id);
                append_skip_log(
                    &crate::routine_storage::routine_rel_dir(routine),
                    now_secs(),
                    &reason,
                );
                return;
            }
            // Overlap guard (#514): a routine has no built-in mutual exclusion between fires, so a
            // run outliving its schedule interval would otherwise pile up concurrent agent sessions
            // all acting on the same target — duplicate PRs/issues, racing pushes. Every fire's tmux
            // session name shares the same `moadim-{slug}-` prefix (see `build_routine_command`); if
            // any of them is still alive, skip this fire instead of launching a second one.
            let session_prefix =
                tmux_session_prefix(&crate::routine_storage::routine_slug(routine));
            if tmux_session_prefix_alive(&session_prefix) {
                let reason = format!(
                    "a previous run (tmux session prefix {session_prefix:?}) is still active \
                     (overlap guard)"
                );
                log::warn!("trigger: routine {:?} skipped — {reason}", routine.id);
                append_skip_log(
                    &crate::routine_storage::routine_rel_dir(routine),
                    now_secs(),
                    &reason,
                );
                return;
            }
            // Global concurrency cap (#335): the overlap guard above only stops one routine from
            // stacking on its own still-running fire — it does nothing to bound how many
            // *different* routines run at once. Cron fires for every routine on a shared schedule
            // (e.g. `*/5 * * * *`) naturally align on the same minute boundary, so an unbounded
            // fan-out can thunder-herd the host (CPU/RAM exhaustion, provider API rate-limit
            // bursts). Counted from actual tmux session liveness — not an in-memory counter, which
            // would drift after a crash — via the same list-sessions seam the overlap guard above
            // uses, just matched against every routine's shared `moadim-` prefix instead of one
            // routine's own. Skips (rather than queues) this fire when at/over the cap: the
            // simpler, lower-risk policy, and consistent with the overlap guard's own
            // skip-with-warning shape above. A cap of `0` (the default) means unlimited, so the check is skipped entirely.
            let live = tmux_session_count(TMUX_SESSION_PREFIX);
            let cap = max_concurrent_runs();
            if cap > 0 && live >= cap {
                let reason = format!(
                    "{live} routine session(s) already running, at or over the global \
                     concurrency cap of {cap} (set {MAX_CONCURRENT_RUNS_ENV} to raise it); this \
                     fire will be retried on its next scheduled tick"
                );
                log::warn!("trigger: routine {:?} skipped — {reason}", routine.id);
                append_skip_log(
                    &crate::routine_storage::routine_rel_dir(routine),
                    now_secs(),
                    &reason,
                );
                return;
            }
            let cmd = build_routine_command(routine, &agent, source);
            // `-lc` (login shell) mirrors the crontab invocation (`/bin/sh -l <run.sh>`), so a
            // manual trigger sources the user's `~/.profile` and the agent gets the same
            // environment whether fired by cron or on demand.
            let mut command = std::process::Command::new(sh_bin());
            command.arg("-lc").arg(&cmd);
            // Reap the child in the background so the short-lived launcher shell does not
            // linger as a zombie for the daemon's lifetime (the trigger stays non-blocking).
            crate::utils::process::spawn_and_reap(command, "routine command");
        }
        Err(err) => {
            let reason = format!("cannot load agent {:?} ({err})", routine.agent);
            log::warn!("trigger: routine {:?} skipped — {reason}", routine.id);
            append_skip_log(
                &crate::routine_storage::routine_rel_dir(routine),
                now_secs(),
                &reason,
            );
        }
    }
}

/// Reap finished, expired run workbenches immediately, returning how many were removed and the
/// bytes freed.
///
/// Runs the same sweep as the hourly background task ([`cleanup_expired_workbenches`]) but on
/// demand, so callers need not wait for the next tick. Still-running sessions are never touched.
pub fn svc_cleanup(store: &RoutineStore) -> CleanupResponse {
    let stats = cleanup_expired_workbenches(store);
    CleanupResponse {
        removed: stats.removed,
        freed_bytes: stats.freed_bytes,
    }
}

/// Return the contents of the newest workbench `agent.log` for routine `id`, plus whether that
/// content is a truncated window rather than the complete file (see [`LogWithMeta`]).
pub fn svc_logs(store: &RoutineStore, id: &str) -> Result<LogWithMeta, AppError> {
    let routine = store
        .lock_recover()
        .get(id)
        .cloned()
        .ok_or(AppError::NotFound)?;
    let slug = crate::routine_storage::routine_slug(&routine);
    let rel_dir = crate::routine_storage::routine_rel_dir(&routine);
    let mut newest: Option<(u64, String)> = None;
    if let Ok(entries) = std::fs::read_dir(workbenches_dir()) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            // Select only this routine's own workbenches by an *exact* slug match.
            // A bare `{slug}-` prefix would also match another routine whose slug
            // begins with this one (e.g. `logs` vs `logs-extra`), leaking that
            // routine's log. Reusing the canonical `{slug}-{ts}` parser also makes
            // "newest" a numeric timestamp comparison rather than a lexicographic
            // one over the whole directory name.
            if let Some((dir_slug, ts)) = parse_workbench_name(&name) {
                if dir_slug == slug && newest.as_ref().is_none_or(|(newest_ts, _)| ts > *newest_ts)
                {
                    newest = Some((ts, name));
                }
            }
        }
    }
    let Some((_, dir)) = newest else {
        return skip_log_fallback(&rel_dir);
    };
    let log_path = workbenches_dir().join(dir).join("agent.log");
    if !log_path.exists() {
        return skip_log_fallback(&rel_dir);
    }
    read_log_tail_with_meta(&log_path).map_err(|_| AppError::Internal)
}

/// Fall back to a routine's `skip.log` (see [`append_skip_log`]) when [`svc_logs`] finds no
/// workbench: a trigger that got skipped before spawning (agent load failure, an oversized inline
/// prompt, the overlap guard, or the concurrency cap) never creates a workbench, so without this
/// `routine_logs` would come back looking identical to a routine that was simply never triggered
/// (#1145). Returns an empty tail when `skip.log` doesn't exist either — a routine really can just
/// have no history yet.
pub(crate) fn skip_log_fallback(slug: &str) -> Result<LogWithMeta, AppError> {
    let skip_log_path = crate::paths::routine_skip_log_path(slug);
    if !skip_log_path.exists() {
        return Ok(LogWithMeta::empty());
    }
    read_log_tail_with_meta(&skip_log_path).map_err(|_| AppError::Internal)
}
