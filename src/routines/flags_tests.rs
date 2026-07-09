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
    assert_eq!(flag.flag_type, "bug");
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
    assert_eq!(flag.flag_type, "bug");
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
    assert_eq!(flag.flag_type, "Missing Auth Check!");
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

#[test]
fn create_flag_propagates_create_dir_failure() {
    use std::os::unix::fs::PermissionsExt as _;

    let _home = TempHome::set();
    let routines_dir = crate::paths::routines_dir();
    std::fs::create_dir_all(&routines_dir).unwrap();
    // Strip write permission so `create_dir_all` inside `create_flag` can't create the
    // routine's own directory (let alone the `flags/` dir nested under it).
    let mut perms = std::fs::metadata(&routines_dir).unwrap().permissions();
    perms.set_mode(0o555);
    std::fs::set_permissions(&routines_dir, perms).unwrap();

    let result = create_flag("r1", "bug", "broken", FlagScope::General);

    // Restore write permission so the temp-home cleanup can remove everything.
    let mut perms = std::fs::metadata(&routines_dir).unwrap().permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(&routines_dir, perms).unwrap();

    assert!(result.is_err());
}

#[test]
fn list_flags_returns_empty_for_missing_dir() {
    let _home = TempHome::set();
    assert!(list_flags("no-such-routine").is_empty());
}

#[test]
fn list_flags_round_trips_type_description_and_scope() {
    let _home = TempHome::set();
    create_flag("r1", "bug", "broken thing", FlagScope::General).unwrap();
    create_flag("r1", "gap", "missing thing", FlagScope::Local).unwrap();

    let flags = list_flags("r1");
    assert_eq!(flags.len(), 2);
    assert!(flags.iter().any(|flag| flag.flag_type == "bug"
        && flag.description == "broken thing"
        && flag.scope == FlagScope::General));
    assert!(flags.iter().any(|flag| flag.flag_type == "gap"
        && flag.description == "missing thing"
        && flag.scope == FlagScope::Local));
}

#[test]
fn list_flags_sorted_oldest_first() {
    let _home = TempHome::set();
    let dir = crate::paths::routine_flags_dir("r1");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("bug-200.md"), "bug\n\nnewer\n").unwrap();
    std::fs::write(dir.join("bug-100.md"), "bug\n\nolder\n").unwrap();

    let flags = list_flags("r1");
    assert_eq!(flags.len(), 2);
    assert_eq!(flags[0].description, "older");
    assert_eq!(flags[1].description, "newer");
}

#[test]
fn list_flags_skips_unparsable_filenames() {
    let _home = TempHome::set();
    let dir = crate::paths::routine_flags_dir("r1");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("not-a-flag.txt"), "junk").unwrap();
    std::fs::write(dir.join("bug-100.md"), "bug\n\nreal\n").unwrap();

    let flags = list_flags("r1");
    assert_eq!(flags.len(), 1);
    assert_eq!(flags[0].description, "real");
}

#[test]
fn list_flags_skips_md_files_that_dont_match_the_flag_shape() {
    let _home = TempHome::set();
    let dir = crate::paths::routine_flags_dir("r1");
    std::fs::create_dir_all(&dir).unwrap();
    // Ends in `.md` (passes the extension check) but has no `-` to split a timestamp off of.
    std::fs::write(dir.join("README.md"), "not a flag").unwrap();
    // Has a `-`, but the token after it isn't a valid timestamp.
    std::fs::write(dir.join("bug-notatimestamp.md"), "bug\n\njunk\n").unwrap();
    std::fs::write(dir.join("bug-100.md"), "bug\n\nreal\n").unwrap();

    let flags = list_flags("r1");
    assert_eq!(flags.len(), 1);
    assert_eq!(flags[0].description, "real");
}

#[test]
fn list_flags_skips_entries_it_cant_read_as_text() {
    let _home = TempHome::set();
    let dir = crate::paths::routine_flags_dir("r1");
    std::fs::create_dir_all(&dir).unwrap();
    // A directory whose name matches the `{type}-{timestamp}.md` shape: it parses fine, but
    // `read_to_string` fails on it (it's not a regular file), so it must be skipped rather
    // than propagating an error out of `list_flags`.
    std::fs::create_dir(dir.join("bug-999.md")).unwrap();
    std::fs::write(dir.join("bug-100.md"), "bug\n\nreal\n").unwrap();

    let flags = list_flags("r1");
    assert_eq!(flags.len(), 1);
    assert_eq!(flags[0].description, "real");
}

#[test]
fn list_flags_defaults_missing_description_to_empty() {
    let _home = TempHome::set();
    let dir = crate::paths::routine_flags_dir("r1");
    std::fs::create_dir_all(&dir).unwrap();
    // A file with no blank-line-separated body: `splitn` yields no second part.
    std::fs::write(dir.join("bug-100.md"), "bug").unwrap();

    let flags = list_flags("r1");
    assert_eq!(flags.len(), 1);
    assert_eq!(flags[0].flag_type, "bug");
    assert_eq!(flags[0].description, "");
}

#[test]
fn resolve_flag_deletes_existing_file() {
    let _home = TempHome::set();
    let flag = create_flag("r1", "bug", "broken", FlagScope::General).unwrap();
    let resolved = resolve_flag("r1", &flag.filename).unwrap();
    assert!(resolved);
    assert!(!crate::paths::routine_flags_dir("r1")
        .join(&flag.filename)
        .exists());
}

#[test]
fn resolve_flag_missing_file_returns_false() {
    let _home = TempHome::set();
    let resolved = resolve_flag("r1", "bug-123.md").unwrap();
    assert!(!resolved);
}

#[test]
fn resolve_flag_propagates_remove_failure() {
    use std::os::unix::fs::PermissionsExt as _;

    let _home = TempHome::set();
    let flag = create_flag("r1", "bug", "broken", FlagScope::General).unwrap();
    let dir = crate::paths::routine_flags_dir("r1");
    // Deleting a file requires write permission on its *containing* directory, not the file
    // itself, so stripping it here forces `remove_file` inside `resolve_flag` to fail.
    let mut perms = std::fs::metadata(&dir).unwrap().permissions();
    perms.set_mode(0o555);
    std::fs::set_permissions(&dir, perms).unwrap();

    let result = resolve_flag("r1", &flag.filename);

    // Restore write permission so the temp-home cleanup can remove everything.
    let mut perms = std::fs::metadata(&dir).unwrap().permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(&dir, perms).unwrap();

    assert!(result.is_err());
}

#[test]
fn resolve_flag_rejects_path_traversal() {
    let _home = TempHome::set();
    create_flag("r1", "bug", "broken", FlagScope::General).unwrap();
    assert!(!resolve_flag("r1", "../routine.toml").unwrap());
    assert!(!resolve_flag("r1", "sub/dir.md").unwrap());
    assert!(!resolve_flag("r1", "sub\\dir.md").unwrap());
    assert!(!resolve_flag("r1", "").unwrap());
    assert!(!resolve_flag("r1", "not-markdown.txt").unwrap());
}
