#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use super::{build, RoutineListQuery};

/// Point `MOADIM_HOME_OVERRIDE` at a fresh, empty temp home for the duration of a test, removing it
/// on drop. `build`/`svc_list` reload the store from disk before serving, so this keeps the read off
/// the developer's real `~/.config/moadim/routines`. Tests in this crate run single-threaded, so the
/// global env mutation is safe.
struct TempHome;

impl TempHome {
    fn set() -> Self {
        let dir = std::env::temp_dir().join(format!(
            "moadim-listroutineslogictest-{}",
            uuid::Uuid::new_v4()
        ));
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
fn build_returns_empty_for_fresh_store() {
    let _home = TempHome::set();
    let store = crate::routines::new_store();
    let list = build(
        &store,
        &crate::paths::routines_dir(),
        &RoutineListQuery::default(),
    );
    assert!(list.is_empty());
}
