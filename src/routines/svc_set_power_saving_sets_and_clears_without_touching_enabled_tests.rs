
#[test]
fn svc_set_power_saving_sets_and_clears_without_touching_enabled() {
    let _home = TempHome::set();
    let store = new_store();
    let routine = make_routine("power-saving-set-id", "Power Saving Set Test ZZZ", 1, 1);
    store
        .lock()
        .unwrap()
        .insert("power-saving-set-id".into(), routine);

    let updated = svc_set_power_saving(&store, "power-saving-set-id", true).unwrap();
    assert!(updated.power_saving);
    assert!(updated.enabled, "enabled must be untouched");
    assert!(
        store
            .lock()
            .unwrap()
            .get("power-saving-set-id")
            .unwrap()
            .power_saving
    );

    let updated = svc_set_power_saving(&store, "power-saving-set-id", false).unwrap();
    assert!(!updated.power_saving);
    assert!(updated.enabled);
}

/// Cover the `write_routine(...).map_err(|_| AppError::Internal)?` branch: every other test in
/// this file only exercises the success path, so that `?`'s error arm never ran. A read-only
/// config dir makes `create_private_dir_all` inside `write_routine` fail, mirroring the technique
/// already used for `lock`/`unlock` in `handlers_tests.rs`.
#[test]
fn svc_set_power_saving_returns_internal_on_write_failure() {
    use std::os::unix::fs::PermissionsExt as _;

    let _home = TempHome::set();
    let store = new_store();
    let routine = make_routine(
        "power-saving-write-fail-id",
        "Power Saving Write Fail Test ZZZ",
        1,
        1,
    );
    store
        .lock()
        .unwrap()
        .insert("power-saving-write-fail-id".into(), routine);

    let config_dir = crate::paths::config_dir();
    std::fs::create_dir_all(&config_dir).unwrap();
    let mut perms = std::fs::metadata(&config_dir).unwrap().permissions();
    perms.set_mode(0o555);
    std::fs::set_permissions(&config_dir, perms).unwrap();

    let result = svc_set_power_saving(&store, "power-saving-write-fail-id", true);

    // Restore write permission so `TempHome::drop` can remove the temp tree.
    let mut perms = std::fs::metadata(&config_dir).unwrap().permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(&config_dir, perms).unwrap();

    assert!(
        matches!(result, Err(AppError::Internal)),
        "expected Internal error when write_routine fails, got {result:?}"
    );
}
