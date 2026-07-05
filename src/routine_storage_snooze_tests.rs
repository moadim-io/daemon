#![allow(clippy::missing_docs_in_private_items)]

use super::*;
use crate::routines::{slugify, Routine};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

fn make_routine(id: &str, title: &str) -> Routine {
    Routine {
        model: None,
        id: id.to_string(),
        schedule: "@daily".to_string(),
        title: title.to_string(),
        agent: "claude".to_string(),
        prompt: "do the thing".to_string(),
        goal: None,
        repositories: vec![],
        machines: vec![],
        enabled: true,
        source: "managed".to_string(),
        created_at: 0,
        updated_at: 0,
        last_manual_trigger_at: None,
        last_scheduled_trigger_at: None,
        snoozed_until: None,
        skip_runs: None,
        tags: vec![],
        ttl_secs: None,
        max_runtime_secs: None,
    }
}

/// A unique, not-yet-created scratch directory under the system temp dir.
fn scratch_dir(tag: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("moadim-rs-{tag}-{}", uuid::Uuid::new_v4()))
}

/// Run `body` with `MOADIM_HOME_OVERRIDE` pointed at a fresh temp home, restoring the previous value
/// and removing the temp home afterwards. Mirrors the seam used by the agent registry tests.
fn with_override_home(body: impl FnOnce(&std::path::Path)) {
    let home = scratch_dir("override-home");
    std::fs::create_dir_all(&home).unwrap();
    let previous = std::env::var_os("MOADIM_HOME_OVERRIDE");
    // SAFETY: tests in this crate run single-threaded per binary; we set and immediately restore the
    // override around this call.
    unsafe {
        std::env::set_var("MOADIM_HOME_OVERRIDE", &home);
    }
    body(&home);
    unsafe {
        match previous {
            Some(value) => std::env::set_var("MOADIM_HOME_OVERRIDE", value),
            None => std::env::remove_var("MOADIM_HOME_OVERRIDE"),
        }
    }
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn repersist_routines_logs_on_write_failure() {
    // A routine whose slug directory path is occupied by a regular file makes write_routine fail,
    // exercising the `log::warn!` failure branch in repersist_routines.
    with_override_home(|_home| {
        let routines = crate::paths::routines_dir();
        std::fs::create_dir_all(&routines).unwrap();

        let id = "rs-repersist-fail-id";
        let title = "Rs Repersist Fail Routine";
        let slug = slugify(title);
        // Block the slug dir with a regular file so create_dir_all fails inside write_routine.
        std::fs::write(routines.join(&slug), "block").unwrap();

        let mut map = HashMap::new();
        map.insert(id.to_string(), make_routine(id, title));
        let store = Arc::new(Mutex::new(map));
        repersist_routines(&store);

        // The write failed and was only logged; the blocking file remains.
        assert!(routines.join(&slug).is_file());
    });
}

// ─── New tests for previously uncovered lines ────────────────────────────────

#[test]
fn load_routine_from_dir_missing_title_returns_none() {
    // Covers L118: `let title = toml.title?;` — a TOML that has schedule and agent
    // but no `title` field causes `load_routine_from_dir` to return `None`.
    with_override_home(|_home| {
        let slug = "rs-no-title-zzz";
        let dir = crate::paths::routine_dir(slug);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            crate::paths::routine_toml_path(slug),
            "schedule = \"@daily\"\nagent = \"claude\"\n",
        )
        .unwrap();
        assert!(load_routine_from_dir(slug).is_none());
    });
}

#[test]
fn load_routine_from_dir_missing_schedule_returns_none() {
    // Covers L124: `schedule: toml.schedule?,` — a TOML with `title` and `agent` but
    // no `schedule` field causes `load_routine_from_dir` to return `None`.
    with_override_home(|_home| {
        let slug = "rs-no-schedule-zzz";
        let dir = crate::paths::routine_dir(slug);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            crate::paths::routine_toml_path(slug),
            "title = \"Rs No Schedule\"\nagent = \"claude\"\n",
        )
        .unwrap();
        assert!(load_routine_from_dir(slug).is_none());
    });
}

#[test]
fn load_routine_from_dir_missing_agent_returns_none() {
    // Covers L126: `agent: toml.agent?,` — a TOML with `title` and `schedule` but no
    // `agent` field causes `load_routine_from_dir` to return `None`.
    with_override_home(|_home| {
        let slug = "rs-no-agent-zzz";
        let dir = crate::paths::routine_dir(slug);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            crate::paths::routine_toml_path(slug),
            "title = \"Rs No Agent\"\nschedule = \"@daily\"\n",
        )
        .unwrap();
        assert!(load_routine_from_dir(slug).is_none());
    });
}

#[cfg(unix)]
#[test]
fn write_routine_fails_on_gitignore_write_error() {
    use std::os::unix::fs::PermissionsExt as _;
    // Covers L202: `std::fs::write(&gitignore, ..)? ` — the dir (and its `prompts/`
    // subdir) already exist but the dir is read-only, and `.gitignore` is absent, so
    // writing it fails and the error is propagated.
    //
    // The `prompts/` subdir must be pre-created: `write_routine` calls
    // `create_dir_all(routine_prompts_dir(&slug))` *before* the `.gitignore` write, and
    // creating a not-yet-existing subdir under a read-only parent fails first, which
    // would exercise that branch instead of the intended gitignore-write branch below.
    with_override_home(|_home| {
        let title = "Rs Gitignore Fail Routine";
        let slug = slugify(title);
        let dir = crate::paths::routine_dir(&slug);
        // Create dir and prompts/ without a .gitignore, then lock the dir.
        std::fs::create_dir_all(crate::paths::routine_prompts_dir(&slug)).unwrap();
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o555)).unwrap();

        let result = write_routine(&make_routine("rs-gitignore-fail-id", title));

        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert!(
            result.is_err(),
            "write_routine should fail when .gitignore cannot be written"
        );
    });
}

#[cfg(unix)]
#[test]
fn write_routine_fails_on_routine_toml_write_error() {
    use std::os::unix::fs::PermissionsExt as _;
    // Covers L185: `atomic_write(&routine_toml_path(&slug), ..)? ` — `.gitignore` exists
    // (so that step is skipped), but the dir is read-only so the atomic write for
    // `routine.toml` (which creates a sibling temp file) fails.
    with_override_home(|_home| {
        let title = "Rs Toml Write Fail Routine";
        let slug = slugify(title);
        let dir = crate::paths::routine_dir(&slug);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            crate::paths::routine_gitignore_path(&slug),
            "*.local.*\n*.log\nrun.sh\n",
        )
        .unwrap();
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o555)).unwrap();

        let result = write_routine(&make_routine("rs-toml-write-fail-id", title));

        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert!(
            result.is_err(),
            "write_routine should fail when routine.toml cannot be written"
        );
    });
}

#[test]
fn write_routine_fails_on_runtime_state_write_error() {
    // Covers L190 and L206: `write_runtime_state(..)? ` and the `atomic_write` inside it.
    // `routine.toml` and `prompt.md` writes succeed, but `state.local.toml` is replaced
    // with a non-empty directory so the atomic rename over it fails.
    with_override_home(|_home| {
        let title = "Rs Runtime State Write Fail Routine";
        let slug = slugify(title);
        let mut routine = make_routine("rs-runtime-state-write-fail-id", title);
        routine.last_manual_trigger_at = Some(12345);

        // Block state.local.toml with a non-empty directory so the atomic rename fails.
        let state_path = crate::paths::routine_state_path(&slug);
        std::fs::create_dir_all(&state_path).unwrap();
        std::fs::write(state_path.join("occupant"), "block").unwrap();

        let result = write_routine(&routine);

        // Restore: remove blocking dir so with_override_home can clean up.
        std::fs::remove_dir_all(&state_path).unwrap();

        assert!(
            result.is_err(),
            "write_routine should fail when state sidecar cannot be written"
        );
    });
}

#[test]
fn write_runtime_state_fails_when_state_file_is_a_directory() {
    // Covers L210: `std::fs::remove_file(&path)?` — when `last_manual_trigger_at` is
    // `None` and the state path is a directory (not a regular file), `remove_file` fails
    // because it can only remove files, not directories.
    with_override_home(|_home| {
        let title = "Rs Remove State Dir Fail Routine";
        let slug = slugify(title);
        let mut routine = make_routine("rs-remove-state-dir-id", title);
        routine.last_manual_trigger_at = None;

        // Write once to create the slug dir and all regular sidecars.
        write_routine(&routine).unwrap();

        // Replace state.local.toml with a directory so remove_file fails.
        let state_path = crate::paths::routine_state_path(&slug);
        std::fs::create_dir_all(&state_path).unwrap();

        let result = write_routine(&routine);

        // Restore before assertions so with_override_home can clean up.
        std::fs::remove_dir_all(&state_path).unwrap();

        assert!(
            result.is_err(),
            "write_routine should fail when state.local.toml is a directory"
        );
    });
}

#[test]
fn snooze_fields_round_trip_through_sidecar_not_routine_toml() {
    // Snooze state is ephemeral/daemon-owned, like last_manual_trigger_at: it lives in the
    // gitignored state.local.toml sidecar, not the tracked routine.toml, and round-trips on load.
    with_override_home(|_home| {
        let title = "Rs Snooze Sidecar Routine";
        let slug = slugify(title);
        let mut routine = make_routine("rs-snooze-sidecar-id", title);
        routine.snoozed_until = Some(999_999);
        write_routine(&routine).unwrap();

        let toml_text = std::fs::read_to_string(crate::paths::routine_toml_path(&slug)).unwrap();
        assert!(
            !toml_text.contains("snoozed_until"),
            "routine.toml must not carry snooze state: {toml_text}"
        );
        let state_text = std::fs::read_to_string(crate::paths::routine_state_path(&slug)).unwrap();
        assert!(state_text.contains("snoozed_until"));

        let loaded = load_routine_from_dir(&slug).unwrap();
        assert_eq!(loaded.snoozed_until, Some(999_999));
        assert_eq!(loaded.skip_runs, None);
    });
}

#[test]
fn skip_runs_round_trips_and_clearing_both_removes_sidecar() {
    with_override_home(|_home| {
        let title = "Rs Skip Runs Sidecar Routine";
        let slug = slugify(title);
        let mut routine = make_routine("rs-skip-runs-sidecar-id", title);
        routine.skip_runs = Some(3);
        write_routine(&routine).unwrap();
        assert!(crate::paths::routine_state_path(&slug).exists());
        assert_eq!(load_routine_from_dir(&slug).unwrap().skip_runs, Some(3));

        routine.skip_runs = None;
        write_routine(&routine).unwrap();
        assert!(
            !crate::paths::routine_state_path(&slug).exists(),
            "sidecar should be removed once no runtime state (trigger or snooze) remains"
        );
        assert_eq!(load_routine_from_dir(&slug).unwrap().skip_runs, None);
    });
}

#[test]
fn append_manual_trigger_log_creates_and_appends() {
    // Each call appends one timestamp line; the log grows and load reads the last line.
    with_override_home(|_home| {
        let title = "Rs Manual Log Append Routine";
        let slug = slugify(title);
        write_routine(&make_routine("rs-manual-log-id", title)).unwrap();

        append_manual_trigger_log(&slug, 100);
        append_manual_trigger_log(&slug, 200);
        append_manual_trigger_log(&slug, 300);

        let log_path = crate::paths::routine_manual_log_path(&slug);
        let text = std::fs::read_to_string(&log_path).unwrap();
        assert_eq!(text, "100\n200\n300\n");
        // load reads the last (most recent) line.
        assert_eq!(
            load_routine_from_dir(&slug).unwrap().last_manual_trigger_at,
            Some(300)
        );
    });
}

#[test]
fn append_manual_trigger_log_warns_on_write_failure() {
    // Pointing the log path at a directory (so open fails) exercises the warn branch and
    // does not panic.
    let dir = scratch_dir("manual-log-fail");
    std::fs::create_dir_all(&dir).unwrap();
    // Create a directory where manual.log would be written, so the open call fails.
    let slug_dir = dir.join("rs-manual-log-fail-routine");
    std::fs::create_dir_all(&slug_dir).unwrap();
    let blocker = slug_dir.join("manual.log");
    std::fs::create_dir_all(&blocker).unwrap();

    // Override home so routine_manual_log_path resolves into our scratch dir.
    let previous = std::env::var_os("MOADIM_HOME_OVERRIDE");
    unsafe {
        std::env::set_var("MOADIM_HOME_OVERRIDE", &dir);
    }
    // Should not panic; just logs a warning.
    append_manual_trigger_log("rs-manual-log-fail-routine", 42);
    unsafe {
        match previous {
            Some(value) => std::env::set_var("MOADIM_HOME_OVERRIDE", value),
            None => std::env::remove_var("MOADIM_HOME_OVERRIDE"),
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn migrate_trigger_logs_from_dir_missing_dir_returns() {
    let missing = scratch_dir("trigger-logs-missing");
    migrate_trigger_logs_from_dir(&missing);
    assert!(!missing.exists());
}

#[test]
fn migrate_trigger_logs_from_dir_migrates_scheduled_and_manual() {
    // A dir with both legacy sidecars: scheduled.local.toml and state.local.toml with a manual
    // timestamp. After migration both log files exist and the TOML sidecar is removed.
    let dir = scratch_dir("trigger-logs-migrate");
    std::fs::create_dir_all(&dir).unwrap();

    // Create a routine dir with a legacy scheduled.local.toml and state.local.toml.
    let routine_dir = dir.join("my-routine");
    std::fs::create_dir_all(&routine_dir).unwrap();
    std::fs::write(
        routine_dir.join("scheduled.local.toml"),
        "last_scheduled_trigger_at = 1111\n",
    )
    .unwrap();
    std::fs::write(
        routine_dir.join("state.local.toml"),
        "last_manual_trigger_at = 2222\n",
    )
    .unwrap();

    migrate_trigger_logs_from_dir(&dir);

    assert!(
        !routine_dir.join("scheduled.local.toml").exists(),
        "legacy toml should be removed"
    );
    let sched_text = std::fs::read_to_string(routine_dir.join("scheduled.log")).unwrap();
    assert_eq!(sched_text, "1111\n");
    let manual_text = std::fs::read_to_string(routine_dir.join("manual.log")).unwrap();
    assert_eq!(manual_text, "2222\n");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn migrate_trigger_logs_from_dir_skips_when_logs_already_exist() {
    // If log files are already present, neither is overwritten and the legacy TOML is left alone.
    let dir = scratch_dir("trigger-logs-skip");
    std::fs::create_dir_all(&dir).unwrap();

    let routine_dir = dir.join("my-routine");
    std::fs::create_dir_all(&routine_dir).unwrap();
    std::fs::write(
        routine_dir.join("scheduled.local.toml"),
        "last_scheduled_trigger_at = 5555\n",
    )
    .unwrap();
    std::fs::write(routine_dir.join("scheduled.log"), "9999\n").unwrap();
    std::fs::write(routine_dir.join("manual.log"), "8888\n").unwrap();
    std::fs::write(
        routine_dir.join("state.local.toml"),
        "last_manual_trigger_at = 7777\n",
    )
    .unwrap();

    migrate_trigger_logs_from_dir(&dir);

    // Existing logs are not overwritten.
    assert_eq!(
        std::fs::read_to_string(routine_dir.join("scheduled.log")).unwrap(),
        "9999\n"
    );
    assert_eq!(
        std::fs::read_to_string(routine_dir.join("manual.log")).unwrap(),
        "8888\n"
    );
    // Legacy TOML is left in place (log already existed, so migration was skipped).
    assert!(routine_dir.join("scheduled.local.toml").exists());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn migrate_trigger_logs_from_dir_skips_non_dirs_and_unparsable() {
    // A plain file in the scan dir and a dir with no parsable TOML are both skipped silently.
    let dir = scratch_dir("trigger-logs-nondir");
    std::fs::create_dir_all(&dir).unwrap();

    std::fs::write(dir.join("loose.txt"), "ignore me").unwrap();
    let routine_dir = dir.join("my-routine");
    std::fs::create_dir_all(&routine_dir).unwrap();
    // No TOML files at all.
    migrate_trigger_logs_from_dir(&dir);

    // Nothing was created, function didn't panic.
    assert!(!routine_dir.join("scheduled.log").exists());
    assert!(!routine_dir.join("manual.log").exists());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn migrate_trigger_logs_from_dir_removes_scheduled_toml_when_no_timestamp() {
    // A `scheduled.local.toml` that has no parsable timestamp (e.g. empty or unparsable) still
    // gets removed — there is no timestamp to seed, so we skip the log write and just clean up.
    let dir = scratch_dir("trigger-logs-no-ts");
    std::fs::create_dir_all(&dir).unwrap();

    let routine_dir = dir.join("my-routine");
    std::fs::create_dir_all(&routine_dir).unwrap();
    std::fs::write(routine_dir.join("scheduled.local.toml"), "").unwrap();

    migrate_trigger_logs_from_dir(&dir);

    // No log written (no timestamp to seed), but the empty TOML was still removed.
    assert!(!routine_dir.join("scheduled.log").exists());
    assert!(!routine_dir.join("scheduled.local.toml").exists());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
#[cfg(unix)]
fn migrate_trigger_logs_from_dir_logs_on_scheduled_write_failure() {
    // When writing scheduled.log fails, a warning is logged and the old TOML is left in place.
    use std::os::unix::fs::PermissionsExt;
    let dir = scratch_dir("trigger-logs-sched-fail");
    std::fs::create_dir_all(&dir).unwrap();

    let routine_dir = dir.join("my-routine");
    std::fs::create_dir_all(&routine_dir).unwrap();
    std::fs::write(
        routine_dir.join("scheduled.local.toml"),
        "last_scheduled_trigger_at = 42\n",
    )
    .unwrap();
    // Block the log write by making the routine dir read-only so fs::write fails.
    std::fs::set_permissions(&routine_dir, std::fs::Permissions::from_mode(0o555)).unwrap();

    migrate_trigger_logs_from_dir(&dir);

    // Restore permissions so cleanup can delete the dir.
    std::fs::set_permissions(&routine_dir, std::fs::Permissions::from_mode(0o755)).unwrap();
    // The old TOML is NOT removed because the write failed (continue branch).
    assert!(routine_dir.join("scheduled.local.toml").exists());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
#[cfg(unix)]
fn migrate_trigger_logs_from_dir_logs_on_manual_write_failure() {
    // When writing manual.log fails, a warning is logged but the function does not crash.
    use std::os::unix::fs::PermissionsExt;
    let dir = scratch_dir("trigger-logs-manual-fail");
    std::fs::create_dir_all(&dir).unwrap();

    let routine_dir = dir.join("my-routine");
    std::fs::create_dir_all(&routine_dir).unwrap();
    // Write state.local.toml with last_manual_trigger_at — note: skip_serializing means the
    // field won't appear in daemon-written state files, but legacy files can have it.
    std::fs::write(
        routine_dir.join("state.local.toml"),
        "last_manual_trigger_at = 77\n",
    )
    .unwrap();
    // Make the routine dir read-only so writing manual.log fails.
    std::fs::set_permissions(&routine_dir, std::fs::Permissions::from_mode(0o555)).unwrap();

    migrate_trigger_logs_from_dir(&dir);

    std::fs::set_permissions(&routine_dir, std::fs::Permissions::from_mode(0o755)).unwrap();
    // Function completed without panic.
    assert!(!routine_dir.join("manual.log").exists());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn migrate_trigger_logs_public_wrapper_runs() {
    // Smoke-test the public wrapper (just needs to not panic; the real work is in the _from_dir variant).
    with_override_home(|_home| {
        migrate_trigger_logs();
    });
}
