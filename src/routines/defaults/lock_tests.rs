//! Regression test for the tombstone-file read-modify-write race fixed by
//! [`super::removed_defaults_lock`]. Split out of `mod_tests.rs` to keep that file under the
//! repo's 500-line-per-file cap.

#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use super::*;

/// Run `body` with `HOME`/`XDG_CONFIG_HOME` redirected at a fresh temp home, restoring the
/// previous values and removing the temp home afterward. Mirrors `mod_tests.rs`'s own
/// `with_redirected_home` helper (duplicated here, like every other split-out `*_tests.rs`
/// sibling in this crate, rather than shared across `#[path]` test modules).
fn with_redirected_home(body: impl FnOnce(&std::path::Path)) {
    let home = std::env::temp_dir().join(format!("moadim-defaults-lock-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&home).unwrap();
    let previous_home = std::env::var_os("HOME");
    let previous_xdg = std::env::var_os("XDG_CONFIG_HOME");
    // SAFETY: tests in this crate run single-threaded per binary; we set and immediately restore
    // the overrides around this call. The spawned threads inside `body` only read these env vars
    // (via `crate::paths`), never mutate them, so there is no concurrent-mutation hazard.
    unsafe {
        std::env::set_var("HOME", &home);
        std::env::set_var("XDG_CONFIG_HOME", home.join(".config"));
    }
    body(&home);
    // SAFETY: single-threaded test execution.
    unsafe {
        match previous_home {
            Some(value) => std::env::set_var("HOME", value),
            None => std::env::remove_var("HOME"),
        }
        match previous_xdg {
            Some(value) => std::env::set_var("XDG_CONFIG_HOME", value),
            None => std::env::remove_var("XDG_CONFIG_HOME"),
        }
    }
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn concurrent_record_removed_default_calls_do_not_clobber_each_other() {
    // Regression test for the tombstone-file read-modify-write race: two threads racing
    // `record_removed_default` for two *different* slugs each read the whole file, mutate the
    // slug set, and write it back in full. Without `removed_defaults_lock()` serializing that
    // span, both threads can read the same (empty) snapshot before either writes, and whichever
    // write lands second silently drops the other thread's slug. A `Barrier` forces both threads
    // to start their read-modify-write span at (as close to) the same instant, so an
    // unsynchronized version of this test flakes/fails; with the lock in place, both slugs always
    // survive regardless of which thread wins the race.
    with_redirected_home(|_home| {
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
        let b1 = std::sync::Arc::clone(&barrier);
        let t1 = std::thread::spawn(move || {
            b1.wait();
            record_removed_default("racer-one");
        });
        let b2 = std::sync::Arc::clone(&barrier);
        let t2 = std::thread::spawn(move || {
            b2.wait();
            record_removed_default("racer-two");
        });
        t1.join().unwrap();
        t2.join().unwrap();

        let slugs = read_removed_defaults();
        assert!(
            slugs.contains("racer-one"),
            "concurrent tombstone for racer-two must not drop racer-one"
        );
        assert!(
            slugs.contains("racer-two"),
            "concurrent tombstone for racer-one must not drop racer-two"
        );
    });
}
