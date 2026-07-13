#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use super::*;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

struct TempHome(std::path::PathBuf);

impl TempHome {
    fn set() -> Self {
        let dir = std::env::temp_dir().join(format!("moadim-svctest-{}", uuid::Uuid::new_v4()));
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

fn make_routine(id: &str, title: &str, created_at: u64, updated_at: u64) -> Routine {
    Routine {
        model: None,
        id: id.to_string(),
        schedule: "@daily".to_string(),
        title: title.to_string(),
        agent: "claude".to_string(),
        prompt: "do the thing".to_string(),
        goal: None,
        repositories: vec![],
        machines: vec![crate::machine::current_machine()],
        enabled: true,
        source: "managed".to_string(),
        auto_pull: true,
        created_at,
        updated_at,
        last_manual_trigger_at: None,
        last_scheduled_trigger_at: None,
        snoozed_until: None,
        skip_runs: None,
        power_saving: false,
        tags: vec![],
        ttl_secs: None,
        max_runtime_secs: None,
    }
}

fn store_with(routines: Vec<Routine>) -> RoutineStore {
    let mut map = HashMap::new();
    for routine in routines {
        map.insert(routine.id.clone(), routine);
    }
    Arc::new(Mutex::new(map))
}

/// Serializes the tests that clear `PATH`, so concurrent service tests never see
/// a stripped environment. The poisoned-lock case is recovered into the guard.
static PATH_GUARD: Mutex<()> = Mutex::new(());

/// Run `body` with an empty `PATH`, restoring the original value afterwards.
///
/// Clearing `PATH` makes the `crontab` and `sh` lookups inside the crontab sync
/// and the trigger spawn fail to launch, exercising their warning branches.
fn with_empty_path(body: impl FnOnce()) {
    let guard = PATH_GUARD
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let saved = std::env::var_os("PATH");
    std::env::set_var("PATH", "");
    body();
    match saved {
        Some(value) => std::env::set_var("PATH", value),
        None => std::env::remove_var("PATH"),
    }
    drop(guard);
}

/// Run `body` with `MOADIM_CRONTAB_BIN` pointed at a shim that succeeds (`crontab -l` prints an
/// empty crontab and exits 0; `crontab -` swallows stdin and exits 0), so the crontab sync returns
/// `Ok` and the non-error branch of `svc_create`/`svc_update`/`svc_delete` is exercised without
/// touching the developer's real crontab. The prior env value is restored and the temp dir removed.
fn with_working_crontab(body: impl FnOnce()) {
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt as _;

    let guard = PATH_GUARD
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let base = std::env::temp_dir().join(format!("moadim-routcronok-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&base).unwrap();
    let script = base.join("crontab-ok.sh");
    std::fs::write(
        &script,
        "#!/bin/sh\nif [ \"$1\" = \"-\" ]; then cat > /dev/null; fi\nexit 0\n",
    )
    .unwrap();
    #[cfg(unix)]
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();

    let saved = std::env::var_os("MOADIM_CRONTAB_BIN");
    std::env::set_var("MOADIM_CRONTAB_BIN", &script);
    body();
    match saved {
        Some(value) => std::env::set_var("MOADIM_CRONTAB_BIN", value),
        None => std::env::remove_var("MOADIM_CRONTAB_BIN"),
    }
    let _ = std::fs::remove_dir_all(&base);
    drop(guard);
}

#[test]
fn svc_rename_machine_no_op_when_names_equal() {
    let _home = TempHome::set();
    // Covers the early-return guard (`old == new`) in `svc_rename_machine`.
    let mut routine = make_routine("rename-noop", "Noop", 1, 1);
    routine.machines = vec!["old-machine".into()];
    let store = store_with(vec![routine]);
    // Must not panic or touch disk when the name does not change.
    svc_rename_machine(&store, "old-machine", "old-machine");
    let lock = store.lock_recover();
    assert_eq!(
        lock["rename-noop"].machines,
        vec!["old-machine".to_string()],
        "store must be unchanged after no-op rename"
    );
}

#[test]
fn svc_rename_machine_replaces_old_name_in_matching_routines() {
    let _home = TempHome::set();
    // Covers the core rename path: filter, map, write, and the Ok branch of the crontab
    // sync guard (using a working crontab shim so `sync_routines_to_crontab` returns Ok,
    // exercising the implicit false-branch at the `if let Err` closing brace).
    let mut routine_a = make_routine("rename-a", "Alpha", 1, 1);
    routine_a.machines = vec!["old-machine".into(), "other".into()];
    let mut routine_b = make_routine("rename-b", "Beta", 2, 2);
    routine_b.machines = vec!["unrelated".into()];
    let store = store_with(vec![routine_a, routine_b]);
    with_working_crontab(|| {
        svc_rename_machine(&store, "old-machine", "new-machine");
    });
    let lock = store.lock_recover();
    assert_eq!(
        lock["rename-a"].machines,
        vec!["new-machine".to_string(), "other".to_string()],
        "old name must be replaced with new name"
    );
    assert_eq!(
        lock["rename-b"].machines,
        vec!["unrelated".to_string()],
        "unrelated routine must not be touched"
    );
}

#[test]
fn svc_rename_machine_skips_sync_when_no_routines_match() {
    let _home = TempHome::set();
    // Covers the `!updated.is_empty()` false branch — no crontab sync when nothing changed.
    let mut routine = make_routine("rename-skip", "Skip", 1, 1);
    routine.machines = vec!["other-machine".into()];
    let store = store_with(vec![routine]);
    // No crontab binary on PATH, but no sync should be attempted either.
    with_empty_path(|| {
        svc_rename_machine(&store, "old-machine", "new-machine");
    });
    let lock = store.lock_recover();
    assert_eq!(
        lock["rename-skip"].machines,
        vec!["other-machine".to_string()],
        "non-matching routine must not be modified"
    );
}

#[test]
fn svc_rename_machine_warns_when_crontab_sync_fails() {
    let _home = TempHome::set();
    // `MOADIM_MACHINE` is forced to "new-machine" so that after the rename the routine
    // targets `current_machine()`, which causes `sync_routines_to_crontab` to invoke the
    // crontab binary. With `PATH` cleared crontab can't be found, so the sync returns an
    // Err and the warn on line 584 fires. The in-memory store must still reflect the new
    // name after this failure.
    let saved_machine = std::env::var_os("MOADIM_MACHINE");
    // SAFETY: single-threaded test harness (RUST_TEST_THREADS=1); restored below.
    unsafe {
        std::env::set_var("MOADIM_MACHINE", "new-machine");
    }
    let mut routine = make_routine("rename-warn", "Warn", 1, 1);
    routine.machines = vec!["old-machine".into()];
    let store = store_with(vec![routine]);
    with_empty_path(|| {
        svc_rename_machine(&store, "old-machine", "new-machine");
    });
    // SAFETY: single-threaded test harness; restoring the original value.
    unsafe {
        match saved_machine {
            Some(val) => std::env::set_var("MOADIM_MACHINE", val),
            None => std::env::remove_var("MOADIM_MACHINE"),
        }
    }
    let lock = store.lock_recover();
    assert_eq!(
        lock["rename-warn"].machines,
        vec!["new-machine".to_string()],
        "in-memory store must reflect new name despite crontab failure"
    );
}

#[test]
fn svc_rename_machine_warns_when_write_fails() {
    // Point MOADIM_HOME_OVERRIDE at a regular file so that `write_routine`'s
    // `create_dir_all` call fails (can't create a directory inside a file).
    // The warn path at lines 575-576 is exercised; the in-memory store is still updated.
    let blocking_file = std::env::temp_dir().join(format!("moadim-block-{}", uuid::Uuid::new_v4()));
    std::fs::write(&blocking_file, b"").expect("create blocking file");
    let saved_home = std::env::var_os("MOADIM_HOME_OVERRIDE");
    // SAFETY: single-threaded test harness (RUST_TEST_THREADS=1); restored below.
    unsafe {
        std::env::set_var("MOADIM_HOME_OVERRIDE", &blocking_file);
    }
    let mut routine = make_routine("rename-wf", "Write Fail", 1, 1);
    routine.machines = vec!["old-machine".into()];
    let store = store_with(vec![routine]);
    // write_routine fails; svc_rename_machine must warn and carry on.
    svc_rename_machine(&store, "old-machine", "new-machine");
    // SAFETY: single-threaded test harness; restoring original value.
    unsafe {
        match saved_home {
            Some(val) => std::env::set_var("MOADIM_HOME_OVERRIDE", val),
            None => std::env::remove_var("MOADIM_HOME_OVERRIDE"),
        }
    }
    let _ = std::fs::remove_file(&blocking_file);
    let lock = store.lock_recover();
    assert_eq!(
        lock["rename-wf"].machines,
        vec!["new-machine".to_string()],
        "in-memory store must reflect new name even when disk write fails"
    );
}
