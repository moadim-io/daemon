//! Manual/scheduled triggers, snooze, cleanup, logs, and flags for routines.

use crate::error::AppError;
use crate::paths::workbenches_dir;
use crate::routine_storage::{append_manual_trigger_log, write_routine};
use crate::utils::lock::LockRecover;
use crate::utils::time::now_secs;

use crate::routines::agents::load_agent_command;
use crate::routines::cleanup::{cleanup_expired_workbenches, parse_workbench_name};
use crate::routines::command::{build_routine_command, inline_prompt_overflow, slugify};
use crate::routines::flags::{self, Flag, FlagScope};
use crate::routines::model::{CleanupResponse, Routine, RoutineStore};

/// Record a manual trigger for `id` and spawn the same command the crontab would run.
pub fn svc_trigger(store: &RoutineStore, id: &str) -> Result<Routine, AppError> {
    if crate::global_lock::is_globally_locked() {
        return Err(AppError::Locked("routines are globally locked".into()));
    }
    let mut lock = store.lock_recover();
    let routine = lock.get_mut(id).ok_or(AppError::NotFound)?;
    let ts = now_secs();
    routine.last_manual_trigger_at = Some(ts);
    let routine = routine.clone();
    drop(lock);
    write_routine(&routine).map_err(|_| AppError::Internal)?;
    append_manual_trigger_log(&crate::routines::slugify(&routine.title), ts);
    spawn_routine_command(&routine);
    Ok(routine)
}

/// Run a routine on its schedule: spawn the command the crontab line invokes, without recording a
/// *manual* trigger.
///
/// This is the daemon-side endpoint that the generated crontab line drives
/// (`moadim schedule trigger <id>`). Unlike [`svc_trigger`] it leaves `last_manual_trigger_at`
/// untouched — the spawned command appends the timestamp to the routine's `scheduled.log` itself,
/// which the daemon reads back on the next load. Keeping the two paths distinct preserves the
/// manual-vs-scheduled distinction the timestamps exist to capture.
///
/// A routine snoozed via [`svc_snooze`] (`snoozed_until` in the future, or `skip_runs` above zero)
/// is skipped here instead of spawned: `snoozed_until` clears itself once elapsed (that fire then
/// runs), `skip_runs` decrements once per skipped fire and clears at zero. [`svc_trigger`] (manual)
/// ignores both fields entirely, by design.
pub fn svc_trigger_scheduled(store: &RoutineStore, id: &str) -> Result<Routine, AppError> {
    if crate::global_lock::is_globally_locked() {
        return Err(AppError::Locked("routines are globally locked".into()));
    }
    let mut lock = store.lock_recover();
    let routine = lock.get_mut(id).ok_or(AppError::NotFound)?;

    if let Some(until) = routine.snoozed_until {
        if now_secs() < until {
            return Err(AppError::Locked(format!("routine snoozed until {until}")));
        }
        routine.snoozed_until = None;
        let routine = routine.clone();
        drop(lock);
        write_routine(&routine).map_err(|_| AppError::Internal)?;
        spawn_routine_command(&routine);
        return Ok(routine);
    }
    if let Some(runs) = routine.skip_runs {
        if runs > 0 {
            routine.skip_runs = (runs > 1).then_some(runs - 1);
            let routine = routine.clone();
            drop(lock);
            write_routine(&routine).map_err(|_| AppError::Internal)?;
            return Err(AppError::Locked(format!(
                "routine snoozed, skipping this scheduled run ({} more to skip)",
                routine.skip_runs.unwrap_or(0)
            )));
        }
    }

    let routine = routine.clone();
    drop(lock);
    spawn_routine_command(&routine);
    Ok(routine)
}

/// Resolve the `sh` executable to invoke for a routine launch.
///
/// Honours the `MOADIM_SH_BIN` environment variable when set, falling back to the platform shell
/// (`sh`) otherwise. The override exists so tests can point the spawn at a shim instead of running
/// a real login shell.
///
/// In **test builds**, when no `MOADIM_SH_BIN` shim is configured this never falls back to the
/// real `sh`: it returns a path that cannot exist, so the spawn fails harmlessly instead of
/// launching a real agent process. This closes the same structural gap `crontab_bin()` in
/// `crate::sync` closes for crontab I/O (issue #175) — a test that forgets to
/// clear `PATH` or shim this binary still cannot execute a real command on the developer's
/// machine (issue #217). Tests that need a working spawn set `MOADIM_SH_BIN` to a shim.
pub(crate) fn sh_bin() -> String {
    if let Ok(bin) = std::env::var("MOADIM_SH_BIN") {
        return bin;
    }
    #[cfg(test)]
    let fallback = "/nonexistent/moadim-test-sh-guard".to_string();
    #[cfg(not(test))]
    let fallback = "sh".to_string();
    fallback
}

/// Set or clear a routine's snooze state, skipping its upcoming *scheduled* fires (see
/// [`svc_trigger_scheduled`]) without touching `enabled` or the crontab. Manual triggers
/// ([`svc_trigger`]) always ignore snooze.
///
/// `snoozed_until` and `skip_runs` are mutually exclusive: passing both `Some` is a
/// [`AppError::BadRequest`]. Passing both `None` clears an active snooze.
pub fn svc_snooze(
    store: &RoutineStore,
    id: &str,
    snoozed_until: Option<u64>,
    skip_runs: Option<u32>,
) -> Result<Routine, AppError> {
    if snoozed_until.is_some() && skip_runs.is_some() {
        return Err(AppError::BadRequest(
            "snoozed_until and skip_runs are mutually exclusive; set only one".into(),
        ));
    }
    let mut lock = store.lock_recover();
    let routine = lock.get_mut(id).ok_or(AppError::NotFound)?;
    routine.snoozed_until = snoozed_until;
    routine.skip_runs = skip_runs;
    let routine = routine.clone();
    drop(lock);
    write_routine(&routine).map_err(|_| AppError::Internal)?;
    Ok(routine)
}

/// Spawn the launch command for `routine` under a login shell, logging (rather than failing) when
/// the agent config cannot be loaded, the composed prompt won't fit in an inlined `{prompt}`
/// argument, or the process cannot be spawned.
///
/// `sh -lc` sources the user's `~/.profile`, so the agent inherits their environment (`GH_TOKEN`,
/// API keys, …) regardless of the minimal environment the daemon (or cron) runs under. Shared by the
/// manual ([`svc_trigger`]) and scheduled ([`svc_trigger_scheduled`]) paths.
fn spawn_routine_command(routine: &Routine) {
    match load_agent_command(&routine.agent) {
        Ok(agent) => {
            // Guard against the silent `execve(E2BIG)` no-op an oversized `{prompt}` argument
            // causes inside the detached tmux session (#443): the OS-level failure never
            // surfaces anywhere, so catch it here instead and skip the launch with a visible
            // warning, the same non-fatal shape as the agent-load-failure arm below.
            if let Some(len) = inline_prompt_overflow(routine, &agent) {
                log::warn!(
                    "trigger: composed prompt for routine {:?} is {len} bytes, over the \
                     inline-argument limit for agent {:?}; skipping launch (would fail silently \
                     inside tmux otherwise) — switch the agent's args to {{prompt_file}} or \
                     shorten the routine's prompt/open flags",
                    routine.id,
                    routine.agent,
                );
                return;
            }
            let cmd = build_routine_command(routine, &agent);
            // `-lc` (login shell) mirrors the crontab invocation (`/bin/sh -l <run.sh>`), so a
            // manual trigger sources the user's `~/.profile` and the agent gets the same
            // environment whether fired by cron or on demand.
            let mut command = std::process::Command::new(sh_bin());
            command.arg("-lc").arg(&cmd);
            // Reap the child in the background so the short-lived launcher shell does not
            // linger as a zombie for the daemon's lifetime (the trigger stays non-blocking).
            crate::utils::process::spawn_and_reap(command, "routine command");
        }
        Err(err) => log::warn!(
            "trigger: cannot load agent {:?} ({}) for routine {:?}",
            routine.agent,
            err,
            routine.id
        ),
    }
}

/// Reap finished, expired run workbenches immediately, returning how many were removed.
///
/// Runs the same sweep as the hourly background task ([`cleanup_expired_workbenches`]) but on
/// demand, so callers need not wait for the next tick. Still-running sessions are never touched.
pub fn svc_cleanup(store: &RoutineStore) -> CleanupResponse {
    CleanupResponse {
        removed: cleanup_expired_workbenches(store),
    }
}

/// Rename every existing workbench directory from `old_slug` to `new_slug`, preserving each run's
/// trigger timestamp (`{old_slug}-{ts}` -> `{new_slug}-{ts}`).
///
/// Called from `svc_update` when a routine's title (and thus slug) changes. Workbenches are keyed
/// by slug, not the routine's stable UUID, so without this migration a rename would strand every
/// prior run under the old slug: [`svc_logs`] (which looks up by *current* slug) would find nothing,
/// and an in-flight run would fall through to the cleanup watchdog's orphan defaults instead of the
/// routine's own `ttl_secs`/`max_runtime_secs` (#267). A failed rename is logged and skipped rather
/// than failing the update itself — this is best-effort history preservation, not a correctness
/// requirement of the rename.
pub(super) fn migrate_workbenches(old_slug: &str, new_slug: &str) {
    let Ok(entries) = std::fs::read_dir(workbenches_dir()) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let Some((dir_slug, ts)) = parse_workbench_name(&name) else {
            continue;
        };
        if dir_slug != old_slug {
            continue;
        }
        let from = workbenches_dir().join(&name);
        let to = workbenches_dir().join(format!("{new_slug}-{ts}"));
        if let Err(err) = std::fs::rename(&from, &to) {
            log::warn!("failed to migrate workbench {name} to {new_slug}-{ts}: {err}");
        }
    }
}

/// Return the contents of the newest workbench `agent.log` for routine `id`.
pub fn svc_logs(store: &RoutineStore, id: &str) -> Result<String, AppError> {
    let routine = store
        .lock_recover()
        .get(id)
        .cloned()
        .ok_or(AppError::NotFound)?;
    let slug = slugify(&routine.title);
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
        return Ok(String::new());
    };
    let log_path = workbenches_dir().join(dir).join("agent.log");
    if !log_path.exists() {
        return Ok(String::new());
    }
    std::fs::read_to_string(&log_path).map_err(|_| AppError::Internal)
}

/// Reject a blank (empty/whitespace-only) flag `type` or `description`.
fn validate_flag_field(field: &str, value: &str) -> Result<(), AppError> {
    if value.trim().is_empty() {
        return Err(AppError::BadRequest(format!(
            "flag {field} must not be empty"
        )));
    }
    Ok(())
}

/// Parse a `scope` string into a [`FlagScope`], returning `400 BadRequest` on unknown values.
/// Mirrors `parse_lock_scope` in `handlers.rs`.
fn parse_flag_scope(scope: &str) -> Result<FlagScope, AppError> {
    match scope {
        "general" => Ok(FlagScope::General),
        "local" => Ok(FlagScope::Local),
        other => Err(AppError::BadRequest(format!(
            "unknown flag scope {other:?}; use \"general\" or \"local\""
        ))),
    }
}

/// Look up a routine by `id` and derive its slug, `NotFound` if it does not exist.
fn routine_and_slug(store: &RoutineStore, id: &str) -> Result<(Routine, String), AppError> {
    let routine = store
        .lock_recover()
        .get(id)
        .cloned()
        .ok_or(AppError::NotFound)?;
    let slug = slugify(&routine.title);
    Ok((routine, slug))
}

/// Raise a new flag against routine `id`. `flag_type` and `description` must be non-blank;
/// `scope` is `"general"` (committed) or `"local"` (gitignored). Refreshes the routine's
/// `prompts/prompt.compiled.md` afterward so the next run's "Open flags" section (see
/// `compose_prompt`) includes it.
pub fn svc_create_flag(
    store: &RoutineStore,
    id: &str,
    flag_type: &str,
    description: &str,
    scope: &str,
) -> Result<Flag, AppError> {
    validate_flag_field("type", flag_type)?;
    validate_flag_field("description", description)?;
    let scope = parse_flag_scope(scope)?;
    let (routine, slug) = routine_and_slug(store, id)?;
    let flag =
        flags::create_flag(&slug, flag_type, description, scope).map_err(|_| AppError::Internal)?;
    write_routine(&routine).map_err(|_| AppError::Internal)?;
    Ok(flag)
}

/// List every open flag raised against routine `id`, oldest first.
pub fn svc_list_flags(store: &RoutineStore, id: &str) -> Result<Vec<Flag>, AppError> {
    let (_, slug) = routine_and_slug(store, id)?;
    Ok(flags::list_flags(&slug))
}

/// Resolve (delete) the flag named `filename` under routine `id`.
///
/// `NotFound` when the routine does not exist, `filename` is unsafe, or names no existing flag.
/// Refreshes `prompts/prompt.compiled.md` afterward so a resolved flag stops appearing in the next
/// run's prompt.
pub fn svc_resolve_flag(store: &RoutineStore, id: &str, filename: &str) -> Result<(), AppError> {
    let (routine, slug) = routine_and_slug(store, id)?;
    let resolved = flags::resolve_flag(&slug, filename).map_err(|_| AppError::Internal)?;
    if !resolved {
        return Err(AppError::NotFound);
    }
    write_routine(&routine).map_err(|_| AppError::Internal)?;
    Ok(())
}
