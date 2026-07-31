#![allow(
    clippy::wildcard_imports,
    clippy::too_many_arguments,
    clippy::needless_pass_by_value,
    dead_code,
    reason = "split-out module preserves existing code while keeping files under the linecheck limit"
)]
use super::*;

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
    assert_eq!(flags[0].category, "bug");
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
