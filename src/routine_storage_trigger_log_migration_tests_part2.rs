
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
