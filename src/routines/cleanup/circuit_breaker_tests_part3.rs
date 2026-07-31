
#[test]
fn record_run_outcome_does_not_retrip_or_overwrite_reason_once_disabled() {
    // Once a routine is already auto-disabled, further failures keep incrementing the streak (an
    // operator inspecting it later can see how bad it got) but must not re-run the disable branch
    // or clobber the original reason with a new one every sweep.
    let _home = TempHome::set();
    let store = new_store();
    let mut routine = make_routine("r1", "Breaker Already Tripped", Some(2));
    routine.consecutive_failures = 2;
    routine.enabled = false;
    routine.auto_disabled_reason = Some("auto-disabled after 2 consecutive failed run(s)".into());
    store.lock().unwrap().insert("r1".into(), routine);

    record_run_outcome(&store, "r1", crate::routines::model::RunStatus::Failed);

    let lock = store.lock().unwrap();
    let routine = lock.get("r1").unwrap();
    assert_eq!(routine.consecutive_failures, 3);
    assert!(!routine.enabled);
}

#[test]
fn record_run_outcome_opts_out_when_threshold_none() {
    let _home = TempHome::set();
    let store = new_store();
    let mut routine = make_routine("r1", "Breaker Opt Out None", None);
    routine.consecutive_failures = 99;
    store.lock().unwrap().insert("r1".into(), routine);

    record_run_outcome(&store, "r1", crate::routines::model::RunStatus::Failed);

    let lock = store.lock().unwrap();
    let routine = lock.get("r1").unwrap();
    assert_eq!(routine.consecutive_failures, 100);
    assert!(routine.enabled, "None threshold must never auto-disable");
}

/// A minimal `crontab` shim wired in via `MOADIM_CRONTAB_BIN` that always succeeds, mirroring the
/// pattern in `sync::routines_sync_tests::CronShim` — used here only to exercise the successful
/// crontab-resync path after an auto-disable, since test builds otherwise never touch a real
/// `crontab` (see `crate::sync::crontab_bin`'s doc comment).
struct OkCronShim {
    script: std::path::PathBuf,
    previous: Option<std::ffi::OsString>,
}

impl OkCronShim {
    fn install() -> Self {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("moadim-cbtest-cron-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let script = dir.join("crontab-ok.sh");
        std::fs::write(&script, "#!/bin/sh\nexit 0\n").unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        let previous = std::env::var_os("MOADIM_CRONTAB_BIN");
        // SAFETY: tests in this crate run single-threaded (RUST_TEST_THREADS=1).
        unsafe {
            std::env::set_var("MOADIM_CRONTAB_BIN", &script);
        }
        Self { script, previous }
    }
}

impl Drop for OkCronShim {
    fn drop(&mut self) {
        // SAFETY: single-threaded test execution.
        unsafe {
            match self.previous.take() {
                Some(value) => std::env::set_var("MOADIM_CRONTAB_BIN", value),
                None => std::env::remove_var("MOADIM_CRONTAB_BIN"),
            }
        }
        let _ = std::fs::remove_dir_all(self.script.parent().unwrap());
    }
}

#[allow(
    clippy::missing_docs_in_private_items,
    reason = "split-out module keeps the file under the linecheck limit"
)]
#[path = "circuit_breaker_tests_part2.rs"]
mod circuit_breaker_tests_part2;
