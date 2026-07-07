#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use super::*;

use crate::routines::{new_store, slugify};

/// Point `MOADIM_HOME_OVERRIDE` at a fresh, empty temp home for the duration of a test, removing the
/// env var and the temp dir on drop. This keeps `svc_create`/`svc_update`/`write_routine` and the
/// other disk-touching paths off the developer's real `~/.moadim`, so a panicking assertion can never
/// leak test routines into the real home. Tests in this crate run single-threaded
/// (`RUST_TEST_THREADS=1`), so the global env mutation is safe.
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

fn create_req_with_title(title: &str) -> CreateRoutineRequest {
    CreateRoutineRequest {
        model: None,
        schedule: "@daily".into(),
        title: title.into(),
        agent: "claude".into(),
        prompt: "p".into(),
        goal: None,
        repositories: vec![],
        machines: vec![crate::machine::current_machine()],
        enabled: true,
        ttl_secs: None,
        max_runtime_secs: None,
        tags: vec![],
    }
}

// ─── Flag tests ───────────────────────────────────────────────────────────────

#[test]
fn svc_create_flag_not_found() {
    let _home = TempHome::set();
    let store = new_store();
    let result = svc_create_flag(&store, "missing", "bug", "desc", "general");
    assert!(matches!(result, Err(AppError::NotFound)));
}

#[test]
fn svc_create_flag_rejects_blank_type_and_description() {
    let _home = TempHome::set();
    let store = new_store();
    let created = svc_create(&store, create_req_with_title("Svc Flag Blank ZZZ")).unwrap();
    let id = created.routine.id.clone();

    assert!(matches!(
        svc_create_flag(&store, &id, "  ", "desc", "general"),
        Err(AppError::BadRequest(_))
    ));
    assert!(matches!(
        svc_create_flag(&store, &id, "bug", "  ", "general"),
        Err(AppError::BadRequest(_))
    ));
}

#[test]
fn svc_create_flag_rejects_unknown_scope() {
    let _home = TempHome::set();
    let store = new_store();
    let created = svc_create(&store, create_req_with_title("Svc Flag Scope ZZZ")).unwrap();
    let id = created.routine.id.clone();

    assert!(matches!(
        svc_create_flag(&store, &id, "bug", "desc", "nowhere"),
        Err(AppError::BadRequest(_))
    ));
}

#[test]
fn svc_create_flag_persists_and_refreshes_prompt() {
    let _home = TempHome::set();
    let store = new_store();
    let title = "Svc Flag Create ZZZ";
    let created = svc_create(&store, create_req_with_title(title)).unwrap();
    let id = created.routine.id.clone();

    let flag = svc_create_flag(&store, &id, "bug", "broken thing", "general").unwrap();
    assert_eq!(flag.flag_type, "bug");
    assert_eq!(flag.description, "broken thing");

    // prompt.compiled.local.md is refreshed with the new open flag so the next run sees it.
    let slug = slugify(title);
    let prompt =
        std::fs::read_to_string(crate::paths::routine_compiled_prompt_path(&slug)).unwrap();
    assert!(prompt.contains("Open flags"));
    assert!(prompt.contains("broken thing"));
}

#[cfg(unix)]
#[test]
fn svc_create_flag_returns_internal_on_create_flag_failure() {
    use std::os::unix::fs::PermissionsExt as _;
    // Covers L790: `flags::create_flag(..).map_err(|_| AppError::Internal)?` in
    // `svc_create_flag`. The routine dir is read-only, so `create_flag`'s own
    // `create_dir_all` for the nested `flags/` dir cannot create it.
    let _home = TempHome::set();
    let title = "Svc Flag Create Mkdir Fail ZZZ";
    let store = new_store();
    let created = svc_create(&store, create_req_with_title(title)).unwrap();
    let id = created.routine.id;

    let dir = crate::paths::routine_dir(&slugify(title));
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o555)).unwrap();

    let result = svc_create_flag(&store, &id, "bug", "broken", "general");

    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert!(matches!(result, Err(AppError::Internal)));
}

#[cfg(unix)]
#[test]
fn svc_create_flag_returns_internal_on_write_failure_after_flag_created() {
    use std::os::unix::fs::PermissionsExt as _;
    // Covers L791: `write_routine(..).map_err(|_| AppError::Internal)?` in
    // `svc_create_flag`, reached only once `create_flag` itself has already
    // succeeded. Pre-create the `flags/` dir so `create_flag`'s own
    // `create_dir_all` is a harmless no-op unaffected by the routine dir's
    // permissions, then make the routine dir read-only so the re-persist of
    // `routine.toml` fails.
    let _home = TempHome::set();
    let title = "Svc Flag Create Write Fail ZZZ";
    let store = new_store();
    let created = svc_create(&store, create_req_with_title(title)).unwrap();
    let id = created.routine.id;

    let slug = slugify(title);
    std::fs::create_dir_all(crate::paths::routine_flags_dir(&slug)).unwrap();
    let dir = crate::paths::routine_dir(&slug);
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o555)).unwrap();

    let result = svc_create_flag(&store, &id, "bug", "broken", "general");

    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert!(matches!(result, Err(AppError::Internal)));
}

#[test]
fn svc_list_flags_not_found() {
    let _home = TempHome::set();
    let store = new_store();
    assert!(matches!(
        svc_list_flags(&store, "missing"),
        Err(AppError::NotFound)
    ));
}

#[test]
fn svc_list_flags_returns_created_flags() {
    let _home = TempHome::set();
    let store = new_store();
    let created = svc_create(&store, create_req_with_title("Svc Flag List ZZZ")).unwrap();
    let id = created.routine.id.clone();
    svc_create_flag(&store, &id, "bug", "d1", "general").unwrap();
    svc_create_flag(&store, &id, "gap", "d2", "local").unwrap();

    let flags = svc_list_flags(&store, &id).unwrap();
    assert_eq!(flags.len(), 2);
}

#[test]
fn svc_resolve_flag_not_found_routine() {
    let _home = TempHome::set();
    let store = new_store();
    assert!(matches!(
        svc_resolve_flag(&store, "missing", "bug-1.md"),
        Err(AppError::NotFound)
    ));
}

#[test]
fn svc_resolve_flag_not_found_flag() {
    let _home = TempHome::set();
    let store = new_store();
    let created = svc_create(&store, create_req_with_title("Svc Flag Resolve Miss ZZZ")).unwrap();
    let id = created.routine.id.clone();
    assert!(matches!(
        svc_resolve_flag(&store, &id, "no-such-flag.md"),
        Err(AppError::NotFound)
    ));
}

#[test]
fn svc_resolve_flag_deletes_and_refreshes_prompt() {
    let _home = TempHome::set();
    let store = new_store();
    let title = "Svc Flag Resolve ZZZ";
    let created = svc_create(&store, create_req_with_title(title)).unwrap();
    let id = created.routine.id.clone();
    let flag = svc_create_flag(&store, &id, "bug", "broken thing", "general").unwrap();

    svc_resolve_flag(&store, &id, &flag.filename).unwrap();

    assert!(svc_list_flags(&store, &id).unwrap().is_empty());
    let slug = slugify(title);
    let prompt =
        std::fs::read_to_string(crate::paths::routine_compiled_prompt_path(&slug)).unwrap();
    assert!(!prompt.contains("Open flags"));
}

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

// ─── sh_bin test-build guard (issue #217) ─────────────────────────────────

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
    unsafe {
        match saved {
            Some(value) => std::env::set_var("MOADIM_SH_BIN", value),
            None => std::env::remove_var("MOADIM_SH_BIN"),
        }
    }
    assert_eq!(bin, "/custom/shim/sh");
}
