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

use uuid::Uuid;

use crate::cron_jobs::normalize_schedule;
use crate::routine_storage::write_routine;
use crate::utils::time::now_secs;

use super::command::slugify;
use super::model::{Routine, RoutineStore};

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

/// Prompt for the daily `moadim` cargo update routine.
const UPDATE_MOADIM_PROMPT: &str = "\
Ensure the locally installed `moadim` cargo package is up to date, and update it if it is not.

Steps:
1. Find the installed version: `cargo install --list | grep '^moadim '` (no output means it is not installed).
2. Find the latest published version on crates.io: `cargo search moadim --limit 1`.
3. If `moadim` is not installed, or the installed version is older than the latest published version, run `cargo install moadim --force` to update it.
4. If it is already on the latest version, make no changes.

Report which versions you found and whether an update was performed.
";

/// Built-in default routines, reconciled onto disk on every startup.
const DEFAULT_ROUTINES: &[DefaultRoutine] = &[DefaultRoutine {
    title: "Update moadim cargo package",
    // Daily at 09:00 local time.
    schedule: "0 9 * * *",
    agent: "claude",
    prompt: UPDATE_MOADIM_PROMPT,
}];

/// Build a concrete [`Routine`] from a [`DefaultRoutine`] spec, stamping `now` as the create/update
/// time and normalizing the schedule. Kept separate from disk/store mutation so it can be unit
/// tested.
fn materialize(spec: &DefaultRoutine, now: u64) -> Routine {
    Routine {
        id: Uuid::new_v4().to_string(),
        schedule: normalize_schedule(spec.schedule),
        title: spec.title.to_string(),
        agent: spec.agent.to_string(),
        prompt: spec.prompt.to_string(),
        repositories: Vec::new(),
        enabled: true,
        source: "managed".to_string(),
        created_at: now,
        updated_at: now,
        last_triggered_at: None,
        ttl_secs: None,
    }
}

/// Reconcile an existing default `cur` against its built-in `spec`, preserving the user's choices.
///
/// Returns `Some(updated)` when a daemon-owned field (schedule, agent, prompt, or the empty
/// repositories list) drifted from the spec and the routine must be rewritten, or `None` when `cur`
/// already matches and no write is needed. The user-owned [`Routine::enabled`] toggle is always
/// carried over from `cur` — so a default the user turned off stays off — as are its `id`,
/// `created_at`, and `last_triggered_at`.
fn reconcile(spec: &DefaultRoutine, cur: &Routine, now: u64) -> Option<Routine> {
    let schedule = normalize_schedule(spec.schedule);
    let up_to_date = cur.schedule == schedule
        && cur.agent == spec.agent
        && cur.prompt == spec.prompt
        && cur.repositories.is_empty();
    if up_to_date {
        return None;
    }
    Some(Routine {
        id: cur.id.clone(),
        schedule,
        title: spec.title.to_string(),
        agent: spec.agent.to_string(),
        prompt: spec.prompt.to_string(),
        repositories: Vec::new(),
        // Never override the user's enable/disable choice on an existing default.
        enabled: cur.enabled,
        source: "managed".to_string(),
        created_at: cur.created_at,
        updated_at: now,
        last_triggered_at: cur.last_triggered_at,
        ttl_secs: cur.ttl_secs,
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
            .lock()
            .unwrap()
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
        store.lock().unwrap().insert(routine.id.clone(), routine);
    }
}

#[cfg(test)]
#[path = "defaults_tests.rs"]
mod defaults_tests;
