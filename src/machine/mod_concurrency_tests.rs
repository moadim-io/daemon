//! `max_concurrent_runs_override` persistence tests (issue #1155), split out of `mod_tests.rs` to
//! keep that file under the repo's line-count gate.

use super::*;

/// Save an env var's prior value and restore it on drop, so a test's override never leaks. Tests in
/// this crate run single-threaded per binary (`RUST_TEST_THREADS=1`), so the global mutation is safe.
struct EnvGuard {
    name: &'static str,
    previous: Option<std::ffi::OsString>,
}

impl EnvGuard {
    /// Set `name` to `value`, remembering the prior value for restoration.
    fn set(name: &'static str, value: &str) -> Self {
        let previous = std::env::var_os(name);
        // SAFETY: single-threaded test execution.
        unsafe { std::env::set_var(name, value) }
        Self { name, previous }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        // SAFETY: single-threaded test execution.
        unsafe {
            match self.previous.take() {
                Some(value) => std::env::set_var(self.name, value),
                None => std::env::remove_var(self.name),
            }
        }
    }
}

/// Create a unique tempdir to use as `MOADIM_HOME_OVERRIDE` for a test.
fn temp_home(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("moadim-machine-{tag}-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).expect("create temp home");
    dir
}

#[test]
fn max_concurrent_runs_override_absent_is_none() {
    let home = temp_home("cap-absent");
    let _home = EnvGuard::set("MOADIM_HOME_OVERRIDE", home.to_str().unwrap());
    assert_eq!(max_concurrent_runs_override(), None);
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn set_max_concurrent_runs_override_then_read_roundtrips() {
    let home = temp_home("cap-roundtrip");
    let _home = EnvGuard::set("MOADIM_HOME_OVERRIDE", home.to_str().unwrap());
    set_max_concurrent_runs_override(Some(7)).expect("write cap override");
    assert_eq!(max_concurrent_runs_override(), Some(7));
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn set_max_concurrent_runs_override_none_clears_it() {
    let home = temp_home("cap-clear");
    let _home = EnvGuard::set("MOADIM_HOME_OVERRIDE", home.to_str().unwrap());
    set_max_concurrent_runs_override(Some(3)).expect("write cap override");
    set_max_concurrent_runs_override(None).expect("clear cap override");
    assert_eq!(max_concurrent_runs_override(), None);
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn set_machine_preserves_existing_concurrency_override() {
    let home = temp_home("cap-preserve-on-name-set");
    let _home = EnvGuard::set("MOADIM_HOME_OVERRIDE", home.to_str().unwrap());
    set_max_concurrent_runs_override(Some(5)).expect("write cap override");
    set_machine("my-box").expect("write machine name");
    // Setting the name must not clobber the previously-persisted cap override.
    assert_eq!(max_concurrent_runs_override(), Some(5));
    assert_eq!(read_machine_file(), Some("my-box".to_string()));
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn set_max_concurrent_runs_override_preserves_existing_machine_name() {
    let home = temp_home("cap-preserve-name");
    let _home = EnvGuard::set("MOADIM_HOME_OVERRIDE", home.to_str().unwrap());
    set_machine("my-box").expect("write machine name");
    set_max_concurrent_runs_override(Some(5)).expect("write cap override");
    // Setting the cap override must not clobber the previously-persisted machine name.
    assert_eq!(read_machine_file(), Some("my-box".to_string()));
    assert_eq!(max_concurrent_runs_override(), Some(5));
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn concurrent_set_machine_and_set_cap_override_do_not_clobber_each_other() {
    // Regression test for the machine.local.toml read-modify-write race: two threads racing
    // `set_machine` and `set_max_concurrent_runs_override` each read the whole file, mutate one
    // field, and write the whole struct back. Without `machine_toml_lock()` serializing that
    // span, both threads can read the same (empty) snapshot before either writes, and whichever
    // write lands second silently drops the other thread's field. A `Barrier` forces both
    // threads to start their read-modify-write span at (as close to) the same instant, so an
    // unsynchronized version of this test flakes/fails; with the lock in place, both fields
    // always survive regardless of which thread wins the race.
    let home = temp_home("concurrent-rmw");
    // Set once on the parent thread before either child spawns; both children only read it
    // (via `machine_config_path()`), so there is no concurrent env-var mutation to race on.
    let _home = EnvGuard::set("MOADIM_HOME_OVERRIDE", home.to_str().unwrap());

    let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
    let b1 = std::sync::Arc::clone(&barrier);
    let t1 = std::thread::spawn(move || {
        b1.wait();
        set_machine("racer-box").expect("set_machine");
    });
    let b2 = std::sync::Arc::clone(&barrier);
    let t2 = std::thread::spawn(move || {
        b2.wait();
        set_max_concurrent_runs_override(Some(9)).expect("set_max_concurrent_runs_override");
    });
    t1.join().unwrap();
    t2.join().unwrap();

    assert_eq!(
        read_machine_file(),
        Some("racer-box".to_string()),
        "concurrent cap-override write must not drop the machine name"
    );
    assert_eq!(
        max_concurrent_runs_override(),
        Some(9),
        "concurrent machine-name write must not drop the cap override"
    );
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn set_max_concurrent_runs_override_returns_err_on_write_failure() {
    let home = temp_home("cap-write-fail");
    let _home = EnvGuard::set("MOADIM_HOME_OVERRIDE", home.to_str().unwrap());
    // Block the write by placing a regular file where the config dir should be.
    let config_dir = home.join(".config").join("moadim");
    std::fs::create_dir_all(config_dir.parent().unwrap()).unwrap();
    std::fs::write(&config_dir, b"").unwrap(); // file, not a dir
    assert!(set_max_concurrent_runs_override(Some(1)).is_err());
    let _ = std::fs::remove_dir_all(&home);
}
