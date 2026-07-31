
impl Drop for EnvGuard {
    fn drop(&mut self) {
        // SAFETY: tests in this crate run single-threaded (RUST_TEST_THREADS=1).
        unsafe {
            match self.previous.take() {
                Some(value) => std::env::set_var(self.name, value),
                None => std::env::remove_var(self.name),
            }
        }
    }
}

impl Drop for CronShim {
    fn drop(&mut self) {
        // SAFETY: single-threaded test harness; restore the saved value.
        unsafe {
            match self.previous.take() {
                Some(value) => std::env::set_var("MOADIM_CRONTAB_BIN", value),
                None => std::env::remove_var("MOADIM_CRONTAB_BIN"),
            }
        }
        let _ = std::fs::remove_dir_all(&self.base);
    }
}

// ─── read_crontab via shim ───────────────────────────────────────────────────

#[test]
fn read_crontab_returns_store_contents_on_success() {
    let shim = CronShim::new(Some("0 0 * * * /bin/existing\n"));
    let result = read_crontab().unwrap();
    assert_eq!(result, "0 0 * * * /bin/existing\n");
    drop(shim);
}

#[test]
fn read_crontab_empty_when_no_crontab() {
    // Shim with no store file reports "no crontab" and exits 1 → empty string.
    let shim = CronShim::new(None);
    let result = read_crontab().unwrap();
    assert_eq!(result, "");
    drop(shim);
}

#[test]
fn read_crontab_errors_on_non_no_crontab_failure() {
    // Shim exits non-zero with stderr that does NOT contain "no crontab".
    let shim = CronShim::failing();
    let err = read_crontab().unwrap_err();
    match err {
        SyncError::CrontabCommand(msg) => assert!(msg.contains("boom"), "unexpected msg: {msg}"),
        SyncError::Io(io) => panic!("expected CrontabCommand, got Io({io})"),
    }
    drop(shim);
}

// ─── write_crontab via shim ──────────────────────────────────────────────────

#[test]
fn write_crontab_persists_content_on_success() {
    let shim = CronShim::new(Some(""));
    write_crontab("hello # moadim-routine:z\n").unwrap();
    assert_eq!(shim.store_contents(), "hello # moadim-routine:z\n");
    drop(shim);
}

#[test]
fn write_crontab_errors_on_non_success_exit() {
    let shim = CronShim::failing();
    let err = write_crontab("anything\n").unwrap_err();
    match err {
        SyncError::CrontabCommand(msg) => assert!(
            msg.contains("exited with"),
            "expected exit message, got: {msg}"
        ),
        SyncError::Io(io) => panic!("expected CrontabCommand, got Io({io})"),
    }
    drop(shim);
}

#[allow(
    clippy::missing_docs_in_private_items,
    reason = "split-out module keeps the file under the linecheck limit"
)]
#[path = "crontab_io_tests_part2.rs"]
mod crontab_io_tests_part2;
