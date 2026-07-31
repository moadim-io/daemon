#![allow(
    clippy::wildcard_imports,
    clippy::too_many_arguments,
    clippy::needless_pass_by_value,
    dead_code,
    reason = "split-out module preserves existing code while keeping files under the linecheck limit"
)]
use super::*;

/// Unix epoch seconds of `schedule`'s next fire after now, in the host's local timezone (matching
/// crontab semantics), reusing the same compiled schedule path as the TTL sweep
/// (`cleanup::ttl::cron_interval_secs`).
///
/// `None` when `enabled` is `false`, the daemon is globally locked (see [`crate::global_lock`]),
/// `schedule` cannot be parsed (e.g. `@reboot`), or it has no upcoming fire.
pub(crate) fn next_run_at(schedules: &[String], enabled: bool) -> Option<u64> {
    if !enabled || crate::global_lock::is_globally_locked() {
        return None;
    }
    let union = crate::utils::cron::compiled_union_many(schedules)?;
    let cron = union.iter().next()?.schedule();
    let next = cron.after(&Local::now()).next()?;
    u64::try_from(next.timestamp()).ok()
}

impl Routine {
    /// Return all configured schedules, falling back to the legacy primary schedule when needed.
    pub fn effective_schedules(&self) -> Vec<String> {
        if self.schedules.is_empty() {
            vec![self.schedule.clone()]
        } else {
            self.schedules.clone()
        }
    }
}

impl RoutineResponse {
    /// Build a response from `routine`, deriving registration status and schedule description.
    pub fn from_routine(routine: Routine) -> Self {
        let rel_path = crate::routine_storage::routine_rel_dir(&routine);
        let slug = crate::routine_storage::routine_slug(&routine);
        let folder = crate::routine_storage::routine_folder(&routine);
        // An agent counts as registered only if its config both exists *and* parses: a
        // present-but-malformed config is silently dropped at crontab-sync time, so reporting it as
        // registered would paint a never-firing routine as healthy. See issue #301.
        let agent_command = load_agent_command(&routine.agent);
        let agent_registered = agent_command.is_ok();
        let agent_command_available = agent_command
            .as_ref()
            .is_ok_and(|agent| agent_command_available(&agent.command));
        // Distinct from `agent_command_available` above: the agent binary itself can resolve while
        // its `setup` step still shells out to something missing (issue #404's `python3` case).
        let agent_setup_available = agent_command
            .as_ref()
            .is_ok_and(|agent| setup_step_available(agent.setup.as_deref()));
        let file_path = routines_dir()
            .join(&rel_path)
            .join("routine.toml")
            .to_string_lossy()
            .into_owned();
        let timezone = local_timezone();
        let schedules = routine.effective_schedules();
        let schedule_description = schedules
            .first()
            .and_then(|schedule| describe_schedule(schedule, timezone.as_deref()));
        let schedule_descriptions = schedules
            .iter()
            .filter_map(|schedule| describe_schedule(schedule, timezone.as_deref()))
            .collect();
        let flag_count = list_flags(&slug).len();
        let next_run_at = next_run_at(&schedules, routine.enabled);
        let is_running = tmux_session_prefix_alive(&tmux_session_prefix(&slug));
        // Key names only — never values. `local_env_keys` reads `routine.local.toml` (if any) and
        // drops the values immediately, so a secret override never survives past this call. See
        // `Routine::env`'s doc comment for why the values themselves are never serialized.
        let mut env_keys: Vec<String> = routine.env.keys().cloned().collect();
        env_keys.extend(crate::routine_storage::local_env_keys(&slug));
        env_keys.sort();
        env_keys.dedup();
        Self {
            routine,
            agent_registered,
            agent_command_available,
            agent_setup_available,
            file_path,
            folder,
            slug,
            rel_path,
            schedule_description,
            schedule_descriptions,
            timezone,
            flag_count,
            next_run_at,
            is_running,
            env_keys,
        }
    }
}

/// Result of an on-demand workbench cleanup sweep.
#[derive(Debug, Clone, Serialize, JsonSchema, utoipa::ToSchema)]
pub struct CleanupResponse {
    /// Number of finished, expired run workbenches removed by this sweep.
    pub removed: usize,
    /// Total disk space reclaimed, in bytes, summed across the removed workbench trees. Additive
    /// field: existing `{"removed": N}` consumers are unaffected.
    pub freed_bytes: u64,
}

/// Routines keyed by ID.
pub type RoutineStore = Arc<Mutex<HashMap<String, Routine>>>;

#[cfg(test)]
pub fn new_store() -> RoutineStore {
    Arc::new(Mutex::new(HashMap::new()))
}

/// Serde default for boolean fields that should default to `true`.
pub(crate) const fn bool_true() -> bool {
    true
}
