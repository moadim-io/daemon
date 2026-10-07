
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
    let id = created.routine.id;
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
    let id = created.routine.id;
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
    let id = created.routine.id;
    let flag = svc_create_flag(&store, &id, "bug", "broken thing", "general").unwrap();

    svc_resolve_flag(&store, &id, &flag.filename).unwrap();

    assert!(svc_list_flags(&store, &id).unwrap().is_empty());
    let slug = slugify(title);
    let prompt =
        std::fs::read_to_string(crate::paths::routine_compiled_prompt_path(&slug)).unwrap();
    assert!(!prompt.contains("Open flags"));
}

// ─── sh_bin test-build guard (issue #217) ─────────────────────────────────

#[allow(
    clippy::missing_docs_in_private_items,
    reason = "split-out module keeps the file under the linecheck limit"
)]
#[path = "svc_resolve_flag_returns_internal_on_resolve_flag_failure_tests.rs"]
mod svc_resolve_flag_returns_internal_on_resolve_flag_failure_tests;

// ─── flags keyed by on-disk rel_dir, not title slug (issue #1514) ─────────

#[allow(
    clippy::missing_docs_in_private_items,
    reason = "split-out module keeps the file under the linecheck limit"
)]
#[path = "svc_flag_folder_rel_dir_tests.rs"]
mod svc_flag_folder_rel_dir_tests;
