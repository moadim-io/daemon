
#[test]
fn svc_create_maps_an_on_disk_slug_collision_to_conflict() {
    // #188: a stale `routine.toml` left on disk (e.g. by a directory `remove_routine_dir` failed
    // to clean up) isn't in the in-memory store, so the store-level collision check above passes.
    // `write_routine`'s own on-disk guard still catches it, and `svc_create` must surface that as
    // a 409 `Conflict` (via `map_write_routine_err`), not a 500 `Internal`.
    let _home = TempHome::set();
    let title = "Svc Create Disk Collision ZZZ";
    let stale = make_routine("stale-on-disk-id", title, 1, 1);
    crate::routine_storage::write_routine(&stale).unwrap();

    let store = new_store();
    let mut req = valid_create_request();
    req.title = title.into();
    let result = svc_create(&store, req);
    match result {
        Err(AppError::Conflict(msg)) => {
            assert!(msg.contains("stale-on-disk-id"), "{msg}");
        }
        other => panic!("expected Conflict, got {other:?}"),
    }
}

#[test]
fn svc_delete_tombstones_a_builtin_default_so_it_is_not_resurrected() {
    // #265: deleting a built-in default (title matches DEFAULT_ROUTINES[0]; kept as a literal
    // here since DEFAULT_ROUTINES is private to the `defaults` submodule) must stick — a later
    // `ensure_default_routines` (simulating the next startup) must not re-create it.
    let _home = TempHome::set();
    let title = "Update moadim cargo package";
    let slug = slugify(title);
    let store = new_store();
    let routine = make_routine("default-del-id", title, 1, 1);
    crate::routine_storage::write_routine(&routine).unwrap();
    store
        .lock()
        .unwrap()
        .insert("default-del-id".into(), routine);

    with_working_crontab(|| {
        svc_delete(&store, "default-del-id").unwrap();
    });

    let restarted_store = new_store();
    crate::routines::ensure_default_routines(&restarted_store);
    assert!(
        !restarted_store
            .lock()
            .unwrap()
            .values()
            .any(|routine| slugify(&routine.title) == slug),
        "a deleted built-in default must not be resurrected on the next startup"
    );
}
