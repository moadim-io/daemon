//! Built-in default routines, seeded on startup when absent.
//!
//! Mirrors [`super::ensure_default_agents`]: on startup the daemon writes any built-in routine whose
//! on-disk `routine.toml` is missing, then inserts it into the in-memory store so the crontab sync
//! schedules it. A routine already present on disk is never overwritten, so user edits are
//! preserved.
//!
//! Re-seeding keys on the absence of the slug's `routine.toml`, so a default deleted while the
//! daemon is stopped is recreated on the next start. Suppressing re-add after an explicit delete
//! (e.g. a "removed defaults" marker) is tracked as a follow-up.

use uuid::Uuid;

use crate::cron_jobs::normalize_schedule;
use crate::paths::routine_toml_path;
use crate::routine_storage::write_routine;
use crate::utils::time::now_secs;

use super::command::slugify;
use super::model::{Routine, RoutineStore};

/// A built-in routine specification, materialized into a [`Routine`] when its `routine.toml` is
/// absent.
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

/// Built-in default routines, written on startup if the slug's `routine.toml` does not exist.
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

/// Seed any missing built-in routines into `store` and onto disk.
///
/// For each [`DEFAULT_ROUTINES`] entry whose slug has no `routine.toml`, this assigns a UUID,
/// persists it (`routine.toml` + `prompt.md` + `.gitignore`), and inserts it into `store` so the
/// subsequent crontab sync schedules it. Best-effort: a write failure is logged and skipped rather
/// than aborting startup. Call once at startup after [`crate::routine_storage::load_store`] and
/// before the crontab sync.
pub fn ensure_default_routines(store: &RoutineStore) {
    for spec in DEFAULT_ROUTINES {
        let slug = slugify(spec.title);
        if routine_toml_path(&slug).exists() {
            continue;
        }
        let routine = materialize(spec, now_secs());
        if let Err(e) = write_routine(&routine) {
            log::warn!(
                "ensure_default_routines: failed to write {:?}: {e}; skipping",
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
