
#[test]
fn reconcile_returns_none_when_machines_already_set_and_otherwise_current() {
    // A correctly seeded routine (non-empty machines, current content) must NOT be rewritten
    // just because reconcile now inspects the machines list.
    let spec = &DEFAULT_ROUTINES[0];
    let cur = materialize(spec, 100);
    assert!(
        !cur.machines.is_empty(),
        "materialize must assign a machine — test pre-condition"
    );
    assert!(
        reconcile(spec, &cur, 200).is_none(),
        "a routine with current content and a non-empty machines list must not trigger a rewrite"
    );
}

#[test]
fn materialize_assigns_non_empty_machines_list() {
    // materialize must always seed the current machine so a freshly created default runs
    // immediately instead of being dormant (#723).
    let spec = &DEFAULT_ROUTINES[0];
    let routine = materialize(spec, 0);
    assert!(
        !routine.machines.is_empty(),
        "materialize must assign the current machine to a freshly seeded default routine"
    );
}

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// A unique, not-yet-created scratch home directory under the system temp dir.
fn scratch_home() -> std::path::PathBuf {
    std::env::temp_dir().join(format!("moadim-defaults-{}", uuid::Uuid::new_v4()))
}

/// Run `body` with `HOME` redirected at a fresh temp home (so `crate::paths` resolves all
/// config/routines paths under it), restoring the previous value and removing the temp home
/// afterwards. The crate's tests run single-threaded, so mutating the process-global `HOME` here is
/// safe. `dirs::home_dir()` — which every `crate::paths` builder consults — reads `$HOME` on this
/// platform, so redirecting it points `routines_dir()` (and thus `write_routine`) at the tempdir.
fn with_redirected_home(body: impl FnOnce(&std::path::Path)) {
    let home = scratch_home();
    std::fs::create_dir_all(&home).unwrap();
    let previous_home = std::env::var_os("HOME");
    let previous_xdg = std::env::var_os("XDG_CONFIG_HOME");
    // SAFETY: tests in this crate run single-threaded per binary; we set and immediately restore the
    // overrides around this call. XDG_CONFIG_HOME is also redirected so config_root() uses the
    // temp home rather than a CI runner's real XDG path.
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

/// An empty in-memory routine store.
fn empty_store() -> RoutineStore {
    Arc::new(Mutex::new(HashMap::new()))
}

#[allow(
    clippy::missing_docs_in_private_items,
    reason = "split-out module keeps the file under the linecheck limit"
)]
#[path = "ensure_default_routines_seeds_empty_store_tests.rs"]
mod ensure_default_routines_seeds_empty_store_tests;

#[allow(
    clippy::missing_docs_in_private_items,
    reason = "split-out module keeps the file under the linecheck limit"
)]
#[path = "clear_removed_default_is_best_effort_on_write_failure_tests.rs"]
mod clear_removed_default_is_best_effort_on_write_failure_tests;
