#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use super::*;

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
    // SAFETY: single-threaded test execution.
    unsafe {
        match previous {
            Some(value) => std::env::set_var("MOADIM_HOME_OVERRIDE", value),
            None => std::env::remove_var("MOADIM_HOME_OVERRIDE"),
        }
    }
    let _ = std::fs::remove_dir_all(&home);
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
