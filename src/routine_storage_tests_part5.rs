
#[test]
fn load_store_includes_written_routine() {
    with_override_home(|_home| {
        let id = "rs-loadstore-id";
        let title = "Rs Loadstore Routine";
        write_routine(&make_routine(id, title)).unwrap();
        let store = load_store();
        assert!(store.lock().unwrap().contains_key(id));
    });
}

#[test]
fn load_store_from_dir_skips_unloadable_dirs() {
    // Covers both `None` arms of the scan loop: a dir whose routine.toml is present but
    // unparsable (warned + skipped) and a dir with no routine.toml at all (skipped quietly).
    // Neither lands in the store, and a valid sibling routine is unaffected.
    with_override_home(|_home| {
        write_routine(&make_routine("rs-valid-id", "Rs Valid Routine")).unwrap();

        let bad_dir = crate::paths::routine_dir("rs-bad-toml");
        std::fs::create_dir_all(&bad_dir).unwrap();
        std::fs::write(
            crate::paths::routine_toml_path("rs-bad-toml"),
            "id = \"x\"\nschedu",
        )
        .unwrap();

        let empty_dir = crate::paths::routine_dir("rs-no-toml");
        std::fs::create_dir_all(&empty_dir).unwrap();

        let store = load_store_from_dir(&crate::paths::routines_dir());
        let guard = store.lock().unwrap();
        assert!(guard.values().any(|routine| routine.id == "rs-valid-id"));
        assert!(!guard.contains_key("rs-bad-toml"));
        assert!(!guard.contains_key("rs-no-toml"));
    });
}

#[test]
fn load_store_from_dir_missing_dir_empty() {
    let store = load_store_from_dir(std::path::Path::new("/nonexistent-routines-dir-99999"));
    assert!(store.lock().unwrap().is_empty());
}

#[test]
fn remove_routine_dir_noop_when_absent() {
    with_override_home(|_home| {
        remove_routine_dir("rs-never-created-zzz").unwrap();
    });
}
