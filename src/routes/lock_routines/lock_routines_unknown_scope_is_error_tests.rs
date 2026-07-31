
#[test]
fn lock_routines_unknown_scope_is_error() {
    let _home = TempHome::set();
    let handler = make_handler();
    let result = handler
        .lock_routines(Parameters(LockRoutinesInput {
            scope: "oops".into(),
        }))
        .unwrap();
    assert!(result.is_error.unwrap_or(false));
}

#[test]
fn lock_routines_succeeds_when_crontab_sync_passes() {
    // Covers the success fall-through `}` of `if let Err(sync_err) = sync_routines_to_crontab`.
    let _home = TempHome::set();
    let _shim = SucceedingCronShim::new();
    let handler = make_handler();
    let result = handler
        .lock_routines(Parameters(LockRoutinesInput {
            scope: "local".into(),
        }))
        .unwrap();
    assert!(!result.is_error.unwrap_or(false));
    crate::global_lock::set_lock(crate::global_lock::LockScope::Local, false).unwrap();
}

#[test]
fn lock_routines_logs_warn_when_crontab_sync_fails() {
    // Covers the `log::warn!("crontab sync after lock failed: ...")` line.
    let _home = TempHome::set();
    let _shim = FailingCronShim::new();
    let handler = make_handler();
    // The lock still succeeds even if the subsequent crontab sync fails.
    let result = handler
        .lock_routines(Parameters(LockRoutinesInput {
            scope: "shared".into(),
        }))
        .unwrap();
    assert!(!result.is_error.unwrap_or(false));
    crate::global_lock::set_lock(crate::global_lock::LockScope::Shared, false).unwrap();
}

#[test]
fn lock_routines_returns_error_when_set_lock_fails() {
    // Covers the `Err(error) => err(error)` IO error path in lock_routines.
    // Make set_lock fail by placing a regular file where the config dir must be created.
    let dir = std::env::temp_dir().join(format!("moadim-lockfail-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    // Write a file at `.config` so create_dir_all(".config/moadim") fails.
    std::fs::write(dir.join(".config"), b"not a dir").unwrap();
    // SAFETY: single-threaded.
    unsafe {
        std::env::set_var("MOADIM_HOME_OVERRIDE", &dir);
    }
    let handler = make_handler();
    let result = handler
        .lock_routines(Parameters(LockRoutinesInput {
            scope: "shared".into(),
        }))
        .unwrap();
    assert!(result.is_error.unwrap_or(false));
    // SAFETY: cleanup.
    unsafe {
        std::env::remove_var("MOADIM_HOME_OVERRIDE");
    }
    let _ = std::fs::remove_dir_all(&dir);
}
