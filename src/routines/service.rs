//! Store-mutating service functions: list, get, create, update, delete, trigger, and logs.

use crate::utils::lock::LockRecover;
use uuid::Uuid;

use crate::error::AppError;
use crate::routine_storage::{remove_routine_dir, routine_rel_dir, routine_slug, write_routine};
use crate::utils::cron::{normalize_schedule, validate_cron};
use crate::utils::time::now_secs;

use super::cleanup::{
    kill_sessions_for_deleted_routine, max_runtime_ceiling_secs, ttl_ceiling_secs,
};
use super::command::slugify;
use super::defaults::{clear_removed_default, is_default_slug, record_removed_default};
#[allow(
    clippy::missing_docs_in_private_items,
    reason = "split-out module keeps the file under the linecheck limit"
)]
#[path = "svc_list.rs"]
mod svc_list;
pub(crate) use svc_list::*;
#[allow(
    clippy::missing_docs_in_private_items,
    reason = "split-out module keeps the file under the linecheck limit"
)]
#[path = "svc_delete.rs"]
mod svc_delete;
pub(crate) use svc_delete::*;

#[cfg(test)]
use super::model::Repository;
use super::model::{
    CreateRoutineRequest, Routine, RoutineListQuery, RoutineResponse, RoutineSort, RoutineStore,
    SortOrder, UpdateRoutineRequest,
};

#[path = "service_validate.rs"]
mod service_validate;
#[cfg(test)]
use service_validate::MAX_TITLE_LEN;
use service_validate::{
    map_write_routine_err, normalize_model, reject_blank, reject_over_ceiling, reject_zero_secs,
    validate_agent, validate_env, validate_goal, validate_machines, validate_prompt,
    validate_repositories, validate_tags, validate_title,
};

/// Validate and normalize all schedules, requiring at least one non-blank expression.
pub(super) fn validate_and_normalize_schedules(
    schedules: &[String],
) -> Result<Vec<String>, AppError> {
    if schedules.is_empty() {
        return Err(AppError::BadRequest(
            "at least one schedule is required".to_string(),
        ));
    }
    let mut normalized = Vec::with_capacity(schedules.len());
    for schedule in schedules {
        reject_blank("schedule", schedule)?;
        validate_cron(schedule)?;
        normalized.push(normalize_schedule(schedule));
    }
    Ok(normalized)
}

/// Return the minimum cron-derived ceiling across every configured schedule.
pub(super) fn min_schedule_ceiling(schedules: &[String], ceiling_for: impl Fn(&str) -> u64) -> u64 {
    schedules
        .iter()
        .map(String::as_str)
        .map(ceiling_for)
        .min()
        .unwrap_or(u64::MAX)
}

/// Sort key placing routines with a repository before those without, then by
/// the primary (first) repository URL alphabetically (case-insensitive).
fn repo_sort_key(routine: &Routine) -> (bool, String) {
    match routine.repositories.first() {
        Some(repo) => (false, repo.repository.to_lowercase()),
        None => (true, String::new()),
    }
}

#[path = "service_update.rs"]
mod service_update;
pub(crate) use service_update::svc_update;
#[path = "service_move.rs"]
mod service_move;
pub(crate) use service_move::{svc_move, MoveRoutineRequest};

#[path = "service_log_tail.rs"]
mod service_log_tail;
#[cfg(test)]
pub(crate) use service_log_tail::{read_log_tail, strip_ansi_noise, MAX_LOG_TAIL_BYTES};
#[cfg(test)]
#[path = "service_log_tail_tests.rs"]
mod service_log_tail_tests;

#[path = "scheduled_trigger_claim.rs"]
mod scheduled_trigger_claim;
#[path = "service_trigger.rs"]
mod service_trigger;
#[cfg(test)]
pub(crate) use service_trigger::{sh_bin, svc_trigger};
pub(crate) use service_trigger::{
    svc_cleanup, svc_logs, svc_set_power_saving, svc_snooze, svc_trigger_scheduled,
    svc_trigger_with_system_power_saving_override,
};

#[path = "service_runs.rs"]
mod service_runs;
pub(crate) use service_runs::{svc_list_all_runs, svc_list_runs};

#[path = "service_run_files.rs"]
mod service_run_files;
pub(crate) use service_run_files::{svc_get_prompt_preview, svc_run_log, svc_run_summary};

#[path = "service_trigger_flags.rs"]
mod service_trigger_flags;
pub(crate) use service_trigger_flags::{svc_create_flag, svc_list_flags, svc_resolve_flag};

#[cfg(test)]
#[path = "service_tests.rs"]
mod service_tests;

#[cfg(test)]
#[path = "service_multi_schedule_tests.rs"]
mod service_multi_schedule_tests;

#[cfg(test)]
#[path = "service_list_tests.rs"]
mod service_list_tests;

#[cfg(test)]
#[path = "service_sync_tests.rs"]
mod service_sync_tests;

#[cfg(test)]
#[path = "service_field_validation_create_tests.rs"]
mod service_field_validation_create_tests;

#[cfg(test)]
#[path = "service_field_validation_update_tests.rs"]
mod service_field_validation_update_tests;

#[cfg(test)]
#[path = "service_flag_tests.rs"]
mod service_flag_tests;

#[cfg(test)]
#[path = "service_rename_machine_tests.rs"]
mod service_rename_machine_tests;

#[cfg(test)]
#[path = "service_model_tests.rs"]
mod service_model_tests;

#[cfg(test)]
#[path = "service_logs_tests.rs"]
mod service_logs_tests;

#[cfg(test)]
#[path = "service_runs_tests.rs"]
mod service_runs_tests;

#[cfg(test)]
#[path = "service_trigger_tests.rs"]
mod service_trigger_tests;

#[cfg(test)]
#[path = "service_power_saving_tests.rs"]
mod service_power_saving_tests;
include!("service_2.rs");
