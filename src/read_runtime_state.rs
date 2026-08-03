#![allow(
    clippy::wildcard_imports,
    clippy::too_many_arguments,
    clippy::needless_pass_by_value,
    dead_code,
    reason = "split-out module preserves existing code while keeping files under the linecheck limit"
)]
use super::*;

/// Read a routine's `state.local.toml` sidecar under `base`, defaulting to an empty
/// [`RuntimeState`] when the sidecar is absent or unparsable (e.g. before the routine has ever
/// been snoozed).
///
/// Base-dir-aware so the `routine_storage_load` loaders can resolve it coherently for any scan
/// root, not only the global [`routines_dir`].
pub(crate) fn read_runtime_state(base: &std::path::Path, dir_name: &str) -> RuntimeState {
    std::fs::read_to_string(base.join(dir_name).join("state.local.toml"))
        .ok()
        .and_then(|text| toml::from_str(&text).ok())
        .unwrap_or_default()
}

/// Read a routine's gitignored `routine.local.toml` sidecar (`{routines_dir}/{id}/routine.local.toml`,
/// see [`crate::paths::routine_local_toml_path`]), defaulting to an empty map when absent or
/// unparsable — mirrors [`read_runtime_state()`].
///
/// **Values only ever flow into [`crate::routines::build_routine_command`]**, which reads
/// this right before it shell-quotes each entry into an `export KEY=value` launch statement. Every
/// other caller (API responses, the UI, logs) must go through [`local_env_keys`] instead, which
/// discards the values this returns and keeps only the key names.
pub(crate) fn read_local_env(id: &str) -> HashMap<String, String> {
    std::fs::read_to_string(crate::paths::routine_local_toml_path(id))
        .ok()
        .and_then(|text| toml::from_str::<RoutineLocalToml>(&text).ok())
        .map(|local| local.env)
        .unwrap_or_default()
}

/// Return only the *key names* set in a routine's `routine.local.toml` sidecar, never their
/// values — the redaction-safe read used by [`crate::routines::RoutineResponse::from_routine`]
/// to surface "what's configured" without ever exposing a secret over the API (#408).
pub(crate) fn local_env_keys(id: &str) -> Vec<String> {
    read_local_env(id).into_keys().collect()
}

/// Write `routine` to disk: `routine.toml` (tracked config), `schedule.cron` (tracked cron entry),
/// the `prompts/prompt.pure.md` (raw) and `prompts/prompt.compiled.local.md` (composed) sidecars,
/// and the gitignored `state.local.toml` runtime sidecar. Gitignore coverage for the machine-local
/// files comes from the single config-dir `.gitignore` (`cli_system::ensure_config_gitignore`),
/// whose patterns apply recursively; per-routine `.gitignore` files are no longer generated. One
/// left behind by an older daemon is not touched — it may carry user-added patterns, and it stays
/// correct alongside the root one.
///
/// Existing routines are written back to their current filesystem location under `routines/`.
/// The title-derived slug is used only for brand-new routines that do not have a persisted
/// `routine.toml` yet. The UUID `id` is stored inside `routine.toml` so it survives a title rename
/// or explicit filesystem move. Daemon-written runtime state goes to the sidecar, not
/// `routine.toml`, so a trigger never churns the version-controlled config file.
///
/// The target directory's `routine.toml`, when present, must belong to the same id. This is the last
/// line of defense against silently overwriting another routine's files (#188).
pub fn write_routine(routine: &Routine) -> std::io::Result<()> {
    let rel_dir = routine_storage_location::routine_rel_dir(routine);
    write_routine_to_rel_dir(routine, &rel_dir)
}

/// Write `routine` to an explicit directory relative to `routines/`.
///
/// Used only by migrations that deliberately move legacy on-disk layouts; normal updates should use
/// [`write_routine`] so filesystem-owned locations are preserved.
pub(crate) fn write_routine_to_rel_dir(routine: &Routine, rel_dir: &str) -> std::io::Result<()> {
    let dir = routine_dir(rel_dir);
    if let Some(existing_id) =
        read_routine_toml(&routine_toml_path(rel_dir)).and_then(|existing| existing.id)
    {
        if existing_id != routine.id {
            return Err(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                format!(
                    "routine dir \"{rel_dir}\" is already used on disk by routine {existing_id}; refusing to overwrite it"
                ),
            ));
        }
    }
    crate::utils::fs_perms::create_private_dir_all(&dir)?;
    crate::utils::fs_perms::create_private_dir_all(&routine_prompts_dir(rel_dir))?;
    if !routine.enabled {
        write_disabled_state(rel_dir, false)?;
    }

    // Remove any stale `run.sh` left by an older daemon that generated per-routine launch scripts;
    // the crontab line now invokes the binary directly, so the script is obsolete. Best-effort: a
    // missing file is fine. Startup re-persists every routine, so this heals existing installs.
    let _ = std::fs::remove_file(routine_script_path(rel_dir));

    let toml_routine = RoutineToml {
        id: Some(routine.id.clone()),
        title: Some(routine.title.clone()),
        agent: Some(routine.agent.clone()),
        model: routine.model.clone(),
        // Never written; the raw prompt now lives in the `prompts/prompt.pure.md` sidecar below.
        prompt: None,
        goal: routine.goal.clone(),
        repositories: routine.repositories.clone(),
        machines: routine.machines.clone(),
        enabled: None,
        power_saving_exempt: routine.power_saving_exempt,
        created_at: Some(routine.created_at),
        updated_at: Some(routine.updated_at),
        // Runtime state is written to the sidecar below, never to the tracked `routine.toml`
        // (`skip_serializing` also keeps this field out regardless of its value).
        last_manual_trigger_at: None,
        ttl_secs: routine.ttl_secs,
        max_runtime_secs: routine.max_runtime_secs,
        failure_threshold: routine.failure_threshold,
        notifications: routine.notifications.clone(),
        tags: routine.tags.clone(),
        env: routine.env.clone(),
    };
    let text = toml::to_string_pretty(&toml_routine).map_err(std::io::Error::other)?;
    // Atomic write (temp + rename) so any concurrent reader never observes a torn routine.toml —
    // a torn file parses to `None` and would silently drop the routine from the store. (Note:
    // there is no continuously-running reverse crontab sync re-reading these files; reverse sync
    // is implemented but not wired up — see issue #218.)
    atomic_write(&routine_toml_path(rel_dir), text.as_bytes())?;
    atomic_write(
        &routine_cron_path(rel_dir),
        format!("{}\n", routine.effective_schedules().join("\n")).as_bytes(),
    )?;
    atomic_write(
        &routine_pure_prompt_path(rel_dir),
        routine.prompt.as_bytes(),
    )?;
    atomic_write(
        &routine_compiled_prompt_path(rel_dir),
        compose_prompt(routine).as_bytes(),
    )?;
    write_runtime_state(rel_dir, routine)?;
    if routine.enabled {
        write_disabled_state(rel_dir, true)?;
    }
    Ok(())
}

/// Persist a routine's runtime state to its gitignored `state.local.toml` sidecar.
///
/// Writes the sidecar (atomically) when any tracked field is set, and removes any stale sidecar
/// when all are `None`, so the on-disk state always mirrors the in-memory routine.
pub(crate) fn write_runtime_state(slug: &str, routine: &Routine) -> std::io::Result<()> {
    let path = routine_state_path(slug);
    if routine.snoozed_until.is_none()
        && routine.skip_runs.is_none()
        && !routine.power_saving
        && routine.consecutive_failures == 0
        && routine.auto_disabled_reason.is_none()
    {
        if path.exists() {
            std::fs::remove_file(&path)?;
        }
        return Ok(());
    }
    let state = RuntimeState {
        // Not written (skip_serializing); stored only to satisfy the struct; reads migrate the
        // legacy value from here into manual.log on first load.
        last_manual_trigger_at: None,
        snoozed_until: routine.snoozed_until,
        skip_runs: routine.skip_runs,
        power_saving: routine.power_saving,
        consecutive_failures: routine.consecutive_failures,
        auto_disabled_reason: routine.auto_disabled_reason.clone(),
    };
    let text = toml::to_string_pretty(&state).map_err(std::io::Error::other)?;
    atomic_write(&path, text.as_bytes())?;
    Ok(())
}
include!("append_manual_trigger_log.rs");
