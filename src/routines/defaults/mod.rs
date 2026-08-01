//! Built-in default routines, seeded and kept current on startup.
//!
//! Mirrors [`super::ensure_default_agents`]: on startup the daemon ensures every built-in routine
//! exists, then inserts it into the in-memory store so the crontab sync schedules it.
//!
//! The daemon **owns** the content of its defaults — schedule, agent, and prompt are refreshed from
//! the built-in spec on every start, so improvements ship on upgrade. The one field the daemon never
//! overrides is [`Routine::enabled`]: a new default is created enabled, but if the user has toggled
//! an existing default off it stays off across restarts.
//!
//! A default that is absent from the store because it was never seeded is (re)created enabled. One
//! that is absent because the user explicitly deleted it stays deleted — [`svc_delete`](
//! super::service::svc_delete) records its slug in the [`removed_default_routines_path`] tombstone
//! file, which [`ensure_default_routines`] consults before re-materializing a missing default.
//! Creating a routine whose title matches a tombstoned default (via [`svc_create`](
//! super::service::svc_create)) clears the tombstone, since that is a deliberate "I want this back"
//! signal.
//!
//! Each built-in routine lives in its own submodule (e.g. [`update_moadim`], [`the_1_percent`]).
//! Adding a new default means a new file + one entry in [`DEFAULT_ROUTINES`].

use std::collections::BTreeSet;
use std::sync::{Mutex, OnceLock};

use crate::utils::lock::LockRecover;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::paths::removed_default_routines_path;
use crate::routine_storage::write_routine;
use crate::utils::cron::normalize_schedule;
use crate::utils::time::now_secs;

use super::command::slugify;
use super::model::{Routine, RoutineStore};

#[allow(
    clippy::missing_docs_in_private_items,
    reason = "split-out module keeps the file under the linecheck limit"
)]
mod write_removed_defaults;
pub(crate) use write_removed_defaults::*;

/// "The 1 Percent" self-improving routines agent.
mod the_1_percent;
/// Weekly token-efficiency audit routine.
mod token_trim;
/// Daily `moadim` cargo package update routine.
mod update_moadim;

/// A built-in routine specification: the daemon-owned content reconciled onto disk each startup.
struct DefaultRoutine {
    /// Human name; slugified to name the routine directory, workbench, and tmux session.
    title: &'static str,
    /// Cron expression (local system timezone). Normalized through [`normalize_schedule`].
    schedule: &'static str,
    /// Agent registry key to launch (must match a config under `~/.config/moadim/agents/`).
    agent: &'static str,
    /// Task prompt handed to the agent.
    prompt: &'static str,
    /// Short (≤5 line) statement of the routine's goal, rendered as a `## Goal` preamble.
    goal: &'static str,
}

/// Built-in default routines, reconciled onto disk on every startup.
const DEFAULT_ROUTINES: &[DefaultRoutine] =
    &[update_moadim::SPEC, the_1_percent::SPEC, token_trim::SPEC];

/// Build a concrete [`Routine`] from a [`DefaultRoutine`] spec, stamping `now` as the create/update
/// time and normalizing the schedule. Kept separate from disk/store mutation so it can be unit
/// tested.
fn materialize(spec: &DefaultRoutine, now: u64) -> Routine {
    let schedule = normalize_schedule(spec.schedule);
    Routine {
        id: Uuid::new_v4().to_string(),
        schedule: schedule.clone(),
        schedules: vec![schedule],
        title: spec.title.to_string(),
        agent: spec.agent.to_string(),
        model: None,
        prompt: spec.prompt.to_string(),
        goal: Some(spec.goal.to_string()),
        repositories: Vec::new(),
        // Self-assign a fresh default to the machine seeding it, so it actually runs out of the box
        // (an empty `machines` list would leave the default dormant on every machine). On a shared
        // config repo the default is seeded once, on whichever machine starts first; the user can
        // reassign it with `moadim routines update`.
        machines: vec![crate::machine::current_machine()],
        enabled: true,
        source: "managed".to_string(),
        created_at: now,
        updated_at: now,
        last_manual_trigger_at: None,
        last_scheduled_trigger_at: None,
        snoozed_until: None,
        skip_runs: None,
        power_saving: false,
        power_saving_exempt: false,
        consecutive_failures: 0,
        auto_disabled_reason: None,
        ttl_secs: None,
        max_runtime_secs: None,
        failure_threshold: None,
        notifications: Default::default(),
        tags: Vec::new(),
        env: std::collections::HashMap::new(),
    }
}
include!("reconcile.rs");
