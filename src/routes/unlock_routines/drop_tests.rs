
impl FailingCronShim {
    fn new() -> Self {
        use std::os::unix::fs::PermissionsExt;
        let base = std::env::temp_dir().join(format!("moadim-fcshim-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&base).unwrap();
        let script = base.join("crontab-fail.sh");
        std::fs::write(&script, "#!/bin/sh\nexit 1\n").unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        let previous = std::env::var_os("MOADIM_CRONTAB_BIN");
        // SAFETY: single-threaded test execution (RUST_TEST_THREADS=1).
        unsafe {
            std::env::set_var("MOADIM_CRONTAB_BIN", &script);
        }
        Self { base, previous }
    }
}

impl Drop for FailingCronShim {
    fn drop(&mut self) {
        // SAFETY: single-threaded test execution.
        unsafe {
            match self.previous.take() {
                Some(val) => std::env::set_var("MOADIM_CRONTAB_BIN", val),
                None => std::env::remove_var("MOADIM_CRONTAB_BIN"),
            }
        }
        let _ = std::fs::remove_dir_all(&self.base);
    }
}

#[test]
fn unlock_routines_succeeds_when_crontab_sync_passes() {
    // Covers the success fall-through `}` of `if let Err(sync_err) = sync_routines_to_crontab`.
    let _home = TempHome::set();
    let _shim = SucceedingCronShim::new();
    let handler = make_handler();
    let result = handler
        .unlock_routines(Parameters(UnlockRoutinesInput {
            scope: "all".into(),
        }))
        .unwrap();
    assert!(!result.is_error.unwrap_or(false));
}

#[test]
fn unlock_routines_logs_warn_when_crontab_sync_fails() {
    // Covers the `log::warn!("crontab sync after unlock failed: ...")` line.
    let _home = TempHome::set();
    let _shim = FailingCronShim::new();
    let handler = make_handler();
    let result = handler
        .unlock_routines(Parameters(UnlockRoutinesInput {
            scope: "all".into(),
        }))
        .unwrap();
    assert!(!result.is_error.unwrap_or(false));
}

#[test]
fn unlock_routines_returns_error_when_set_lock_fails() {
    // Covers the `Err(error) => err(error)` IO error path in unlock_routines.
    // Create the sentinel path as a DIRECTORY instead of a file: `path.exists()` is true but
    // `std::fs::remove_file` returns EISDIR, triggering the error return.
    let dir = std::env::temp_dir().join(format!("moadim-unlockfail-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(dir.join(".config").join("moadim")).unwrap();
    // SAFETY: single-threaded.
    unsafe {
        std::env::set_var("MOADIM_HOME_OVERRIDE", &dir);
    }
    // Make the shared sentinel path a directory so remove_file fails.
    let lock_path = crate::paths::global_lock_path();
    std::fs::create_dir_all(&lock_path).unwrap();

    let handler = make_handler();
    let result = handler
        .unlock_routines(Parameters(UnlockRoutinesInput {
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
