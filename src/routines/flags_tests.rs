#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use super::*;

/// Point `MOADIM_HOME_OVERRIDE` at a fresh, empty temp home for the duration of a test, restoring
/// the env var and removing the temp dir on drop. Mirrors `service_tests::TempHome`; tests in this
/// crate run single-threaded (`RUST_TEST_THREADS=1`), so the global env mutation is safe.
struct TempHome(std::path::PathBuf);

impl TempHome {
    fn set() -> Self {
        let dir = std::env::temp_dir().join(format!("moadim-flagstest-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("create temp home");
        // SAFETY: single-threaded test execution.
        unsafe {
            std::env::set_var("MOADIM_HOME_OVERRIDE", &dir);
        }
        Self(dir)
    }
}

impl Drop for TempHome {
    fn drop(&mut self) {
        // SAFETY: single-threaded test execution.
        unsafe {
            std::env::remove_var("MOADIM_HOME_OVERRIDE");
        }
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn create_flag_writes_general_file_with_md_suffix() {
    let _home = TempHome::set();
    let flag = create_flag("r1", "bug", "the thing is broken", FlagScope::General).unwrap();
    assert!(flag.filename.starts_with("bug-"));
    assert!(std::path::Path::new(&flag.filename)
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("md")));
    assert!(!flag.filename.ends_with(".local.md"));
    assert_eq!(flag.category, "bug");
    assert_eq!(flag.description, "the thing is broken");
    assert_eq!(flag.scope, FlagScope::General);
    assert!(crate::paths::routine_flags_dir("r1")
        .join(&flag.filename)
        .exists());
}

#[test]
fn create_flag_writes_local_file_with_local_md_suffix() {
    let _home = TempHome::set();
    let flag = create_flag("r1", "gap", "missing context", FlagScope::Local).unwrap();
    assert!(flag.filename.ends_with(".local.md"));
    assert_eq!(flag.scope, FlagScope::Local);
}

#[test]
fn create_flag_trims_type_and_description() {
    let _home = TempHome::set();
    let flag = create_flag("r1", "  bug  ", "  broken  ", FlagScope::General).unwrap();
    assert_eq!(flag.category, "bug");
    assert_eq!(flag.description, "broken");
}

#[test]
fn create_flag_slugifies_type_in_filename_but_keeps_exact_type_in_body() {
    let _home = TempHome::set();
    let flag = create_flag(
        "r1",
        "Missing Auth Check!",
        "no auth on this route",
        FlagScope::General,
    )
    .unwrap();
    assert!(flag.filename.starts_with("missing-auth-check-"));
    assert_eq!(flag.category, "Missing Auth Check!");
}

#[test]
fn create_flag_bumps_timestamp_on_collision() {
    let _home = TempHome::set();
    let dir = crate::paths::routine_flags_dir("r1");
    std::fs::create_dir_all(&dir).unwrap();
    // Pre-seed a file that collides with whatever `now_secs()` resolves to right now.
    let now = crate::utils::time::now_secs();
    std::fs::write(dir.join(format!("bug-{now}.md")), "bug\n\nfirst\n").unwrap();

    let flag = create_flag("r1", "bug", "second", FlagScope::General).unwrap();
    assert_ne!(flag.filename, format!("bug-{now}.md"));
    assert!(flag.created_at >= now);
    // Both files must exist — the second write must not have clobbered the first.
    assert!(dir.join(format!("bug-{now}.md")).exists());
    assert!(dir.join(&flag.filename).exists());
}

#[test]
fn concurrent_create_flag_calls_do_not_clobber_each_other() {
    // Regression test for the flags/ collision-check-then-write race: two threads racing
    // `create_flag` for the same routine and type each read the directory for a free filename,
    // then write. Without `flags_lock()` serializing that span, both threads can observe the
    // same candidate filename as free before either writes, and whichever write lands second
    // silently clobbers the first. A `Barrier` forces both threads to start their
    // check-then-write span at (as close to) the same instant, so an unsynchronized version of
    // this test flakes/fails; with the lock in place, both flags always survive.
    let _home = TempHome::set();

    let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
    let b1 = std::sync::Arc::clone(&barrier);
    let t1 = std::thread::spawn(move || {
        b1.wait();
        create_flag("r1", "bug", "first", FlagScope::General).unwrap()
    });
    let b2 = std::sync::Arc::clone(&barrier);
    let t2 = std::thread::spawn(move || {
        b2.wait();
        create_flag("r1", "bug", "second", FlagScope::General).unwrap()
    });
    let flag1 = t1.join().unwrap();
    let flag2 = t2.join().unwrap();

    assert_ne!(
        flag1.filename, flag2.filename,
        "concurrent create_flag calls must not resolve to the same filename"
    );
    let created = list_flags("r1");
    assert_eq!(created.len(), 2, "both flags must survive on disk");
    assert!(created.iter().any(|flag| flag.description == "first"));
    assert!(created.iter().any(|flag| flag.description == "second"));
}

#[test]
fn create_flag_propagates_write_failure() {
    use std::os::unix::fs::PermissionsExt as _;

    let _home = TempHome::set();
    let dir = crate::paths::routine_flags_dir("r1");
    std::fs::create_dir_all(&dir).unwrap();
    // Strip write permission so the `atomic_write` inside `create_flag` fails.
    let mut perms = std::fs::metadata(&dir).unwrap().permissions();
    perms.set_mode(0o555);
    std::fs::set_permissions(&dir, perms).unwrap();

    let result = create_flag("r1", "bug", "broken", FlagScope::General);

    // Restore write permission so `TempHome::drop`'s `remove_dir_all` can clean up.
    let mut perms = std::fs::metadata(&dir).unwrap().permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(&dir, perms).unwrap();

    assert!(result.is_err());
}
include!("create_flag_propagates_create_dir_failure_tests.rs");
