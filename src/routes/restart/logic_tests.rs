#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use super::build;

/// Point `MOADIM_HOME_OVERRIDE` at a fresh, empty temp home for the duration of a test, removing it
/// on drop, so the detached restart helper's log file doesn't land in the real home. Tests in this
/// crate run single-threaded per binary, so the global env mutation is safe.
struct TempHome;

impl TempHome {
    fn set() -> Self {
        let dir =
            std::env::temp_dir().join(format!("moadim-restartlogictest-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("create temp home");
        // SAFETY: single-threaded test execution.
        unsafe {
            std::env::set_var("MOADIM_HOME_OVERRIDE", &dir);
        }
        Self
    }
}

impl Drop for TempHome {
    fn drop(&mut self) {
        // SAFETY: single-threaded test execution.
        unsafe {
            std::env::remove_var("MOADIM_HOME_OVERRIDE");
        }
    }
}

#[test]
fn build_spawns_helper_and_acknowledges() {
    // The helper is a detached `current_exe --background` process; under the test harness that exe
    // is the test binary, which rejects `--background` and exits at once, so no real server starts.
    let _home = TempHome::set();
    let response = build().unwrap();
    assert_eq!(response.status, "restarting");
    assert!(response.helper_pid > 0);
}
