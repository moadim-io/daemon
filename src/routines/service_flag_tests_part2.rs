#![allow(
    clippy::wildcard_imports,
    clippy::too_many_arguments,
    clippy::needless_pass_by_value,
    dead_code,
    reason = "split-out module preserves existing code while keeping files under the linecheck limit"
)]
use super::*;

#[cfg(unix)]
#[test]
fn svc_resolve_flag_returns_internal_on_resolve_flag_failure() {
    use std::os::unix::fs::PermissionsExt as _;
    // Covers L808: `flags::resolve_flag(..).map_err(|_| AppError::Internal)?` in
    // `svc_resolve_flag`. The flags dir (not the routine dir) is made read-only,
    // so `remove_file` for the flag can't remove an entry from its parent dir.
    let _home = TempHome::set();
    let title = "Svc Flag Resolve Rm Fail ZZZ";
    let store = new_store();
    let created = svc_create(&store, create_req_with_title(title)).unwrap();
    let id = created.routine.id;
    let flag = svc_create_flag(&store, &id, "bug", "broken", "general").unwrap();

    let flags_dir = crate::paths::routine_flags_dir(&slugify(title));
    std::fs::set_permissions(&flags_dir, std::fs::Permissions::from_mode(0o555)).unwrap();

    let result = svc_resolve_flag(&store, &id, &flag.filename);

    std::fs::set_permissions(&flags_dir, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert!(matches!(result, Err(AppError::Internal)));
}

#[cfg(unix)]
#[test]
fn svc_resolve_flag_returns_internal_on_write_failure_after_flag_resolved() {
    use std::os::unix::fs::PermissionsExt as _;
    // Covers L812: `write_routine(..).map_err(|_| AppError::Internal)?` in
    // `svc_resolve_flag`, reached only once `resolve_flag` itself has already
    // succeeded. Only the routine dir (not the flags dir) is made read-only, so
    // removing the flag file still works but re-persisting `routine.toml` fails.
    let _home = TempHome::set();
    let title = "Svc Flag Resolve Write Fail ZZZ";
    let store = new_store();
    let created = svc_create(&store, create_req_with_title(title)).unwrap();
    let id = created.routine.id;
    let flag = svc_create_flag(&store, &id, "bug", "broken", "general").unwrap();

    let dir = crate::paths::routine_dir(&slugify(title));
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o555)).unwrap();

    let result = svc_resolve_flag(&store, &id, &flag.filename);

    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert!(matches!(result, Err(AppError::Internal)));
}

#[test]
fn sh_bin_never_resolves_to_real_sh_in_test_builds() {
    // Structural guard for issue #217: in a test build, with no `MOADIM_SH_BIN` shim
    // configured, `sh_bin()` must never fall back to the real `sh`, so a test that forgets
    // to clear `PATH` (or shim this binary) cannot launch a real agent process. The
    // resolved path must also not exist, so the eventual spawn fails harmlessly.
    let saved = std::env::var_os("MOADIM_SH_BIN");
    // SAFETY: single-threaded test harness (RUST_TEST_THREADS=1); restored below.
    unsafe {
        std::env::remove_var("MOADIM_SH_BIN");
    }
    let bin = sh_bin();
    // SAFETY: single-threaded test execution.
    unsafe {
        match saved {
            Some(value) => std::env::set_var("MOADIM_SH_BIN", value),
            None => std::env::remove_var("MOADIM_SH_BIN"),
        }
    }
    assert_ne!(bin, "sh", "test build must not fall back to the real sh");
    assert!(
        !std::path::Path::new(&bin).exists(),
        "test-build sh_bin() fallback must not resolve to a real executable"
    );
}

#[test]
fn sh_bin_honors_override() {
    let saved = std::env::var_os("MOADIM_SH_BIN");
    // SAFETY: single-threaded test harness (RUST_TEST_THREADS=1); restored below.
    unsafe {
        std::env::set_var("MOADIM_SH_BIN", "/custom/shim/sh");
    }
    let bin = sh_bin();
    // SAFETY: single-threaded test execution.
    unsafe {
        match saved {
            Some(value) => std::env::set_var("MOADIM_SH_BIN", value),
            None => std::env::remove_var("MOADIM_SH_BIN"),
        }
    }
    assert_eq!(bin, "/custom/shim/sh");
}
