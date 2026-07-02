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
//! A default that is absent from the store (never seeded, or deleted while the daemon was stopped)
//! is (re)created enabled. Suppressing re-add after an explicit delete (e.g. a "removed defaults"
//! marker) is tracked as a follow-up.
//!
//! Each built-in routine lives in its own submodule (e.g. [`update_moadim`], [`the_1_percent`]).
//! Adding a new default means a new file + one entry in [`DEFAULT_ROUTINES`].

use crate::utils::lock::LockRecover;
use uuid::Uuid;

use crate::routine_storage::write_routine;
use crate::utils::cron::normalize_schedule;
use crate::utils::time::now_secs;

use super::command::slugify;
use super::model::{Routine, RoutineStore};

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
}

/// Built-in default routines, reconciled onto disk on every startup.
const DEFAULT_ROUTINES: &[DefaultRoutine] =
    &[update_moadim::SPEC, the_1_percent::SPEC, token_trim::SPEC];

/// Build a concrete [`Routine`] from a [`DefaultRoutine`] spec, stamping `now` as the create/update
/// time and normalizing the schedule. Kept separate from disk/store mutation so it can be unit
/// tested.
fn materialize(spec: &DefaultRoutine, now: u64) -> Routine {
    Routine {
        id: Uuid::new_v4().to_string(),
        schedule: normalize_schedule(spec.schedule),
        title: spec.title.to_string(),
        agent: spec.agent.to_string(),
        model: None,
        prompt: spec.prompt.to_string(),
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
        ttl_secs: None,
        max_runtime_secs: None,
        tags: Vec::new(),
    }
}

/// Reconcile an existing default `cur` against its built-in `spec`, preserving the user's choices.
///
/// Returns `Some(updated)` when a daemon-owned field (schedule, agent, prompt, or the empty
/// repositories list) drifted from the spec and the routine must be rewritten, or `None` when `cur`
/// already matches and no write is needed. The user-owned [`Routine::enabled`] toggle is always
/// carried over from `cur` — so a default the user turned off stays off — as are its `id`,
/// `created_at`, `last_manual_trigger_at`, `last_scheduled_trigger_at`, and `tags`.
///
/// Special case: if `cur.machines` is empty the routine is dormant and can never run. This is the
/// legacy state for defaults seeded before machine-awareness was added. To repair it, an empty
/// machines list is treated as a drift trigger and replaced with the current machine, matching what
/// [`materialize`] does for freshly created defaults. (#723)
fn reconcile(spec: &DefaultRoutine, cur: &Routine, now: u64) -> Option<Routine> {
    let schedule = normalize_schedule(spec.schedule);
    let up_to_date = cur.schedule == schedule
        && cur.agent == spec.agent
        && cur.prompt == spec.prompt
        && cur.repositories.is_empty()
        // An empty machines list means the routine can never run; treat it as drift so the
        // current machine is seeded and the routine becomes active again (#723).
        && !cur.machines.is_empty();
    if up_to_date {
        return None;
    }
    Some(Routine {
        id: cur.id.clone(),
        schedule,
        title: spec.title.to_string(),
        agent: spec.agent.to_string(),
        // Model is user-owned, like `tags`: never overridden by the spec.
        model: cur.model.clone(),
        prompt: spec.prompt.to_string(),
        repositories: Vec::new(),
        // Machine targeting is user-owned, like `enabled`: carry the existing choice across a
        // spec-driven reconcile so a default reassigned (or unassigned) by the user stays that
        // way. Exception: an empty list means the routine is dormant (legacy pre-machine-awareness
        // state); seed the current machine so it starts running out of the box (#723).
        machines: if cur.machines.is_empty() {
            vec![crate::machine::current_machine()]
        } else {
            cur.machines.clone()
        },
        enabled: cur.enabled,
        source: "managed".to_string(),
        created_at: cur.created_at,
        updated_at: now,
        last_manual_trigger_at: cur.last_manual_trigger_at,
        last_scheduled_trigger_at: cur.last_scheduled_trigger_at,
        ttl_secs: cur.ttl_secs,
        max_runtime_secs: cur.max_runtime_secs,
        // Tags are user-owned, like `enabled`: never overridden by the spec.
        tags: cur.tags.clone(),
    })
}

/// Ensure every built-in default routine exists and matches its spec, then schedule it.
///
/// For each [`DEFAULT_ROUTINES`] entry: if a routine with the same slug is already in `store`, it is
/// refreshed via [`reconcile`] (daemon-owned content updated, the user's `enabled` toggle preserved)
/// and only rewritten when it drifted; otherwise a fresh enabled routine is created. Persists each
/// affected routine (`routine.toml` + `prompt.md` + `.gitignore`) and inserts it into `store` so the
/// subsequent crontab sync schedules it. Best-effort: a write failure is logged and skipped rather
/// than aborting startup. Call once at startup after [`crate::routine_storage::load_store`] and
/// before the crontab sync.
pub fn ensure_default_routines(store: &RoutineStore) {
    for spec in DEFAULT_ROUTINES {
        let slug = slugify(spec.title);
        let existing = store
            .lock_recover()
            .values()
            .find(|routine| slugify(&routine.title) == slug)
            .cloned();
        let routine = match existing {
            Some(cur) => match reconcile(spec, &cur, now_secs()) {
                Some(updated) => updated,
                None => continue,
            },
            None => materialize(spec, now_secs()),
        };
        if let Err(err) = write_routine(&routine) {
            log::warn!(
                "ensure_default_routines: failed to write {:?}: {err}; skipping",
                spec.title
            );
            continue;
        }
        store.lock_recover().insert(routine.id.clone(), routine);
    }
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod defaults_tests;
