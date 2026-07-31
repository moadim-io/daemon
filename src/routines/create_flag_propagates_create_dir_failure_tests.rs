
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
    assert!(flags.iter().any(|flag| flag.category == "bug"
        && flag.description == "broken thing"
        && flag.scope == FlagScope::General));
    assert!(flags.iter().any(|flag| flag.category == "gap"
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

#[allow(
    clippy::missing_docs_in_private_items,
    reason = "split-out module keeps the file under the linecheck limit"
)]
#[path = "list_flags_skips_entries_it_cant_read_as_text_tests.rs"]
mod list_flags_skips_entries_it_cant_read_as_text_tests;
