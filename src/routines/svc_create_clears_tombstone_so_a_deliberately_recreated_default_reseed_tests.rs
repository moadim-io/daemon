#![allow(
    clippy::wildcard_imports,
    clippy::too_many_arguments,
    clippy::needless_pass_by_value,
    dead_code,
    reason = "split-out module preserves existing code while keeping files under the linecheck limit"
)]
use super::*;

#[test]
fn svc_create_clears_tombstone_so_a_deliberately_recreated_default_reseeds() {
    // #265: re-creating a routine under a tombstoned default's title is a deliberate "bring it
    // back" signal — the next `ensure_default_routines` should treat it as a normal existing
    // routine again (and, if it's later deleted with no re-create, resume being tombstoned).
    let _home = TempHome::set();
    let title = "Update moadim cargo package";
    let slug = slugify(title);
    record_removed_default(&slug);
    assert!(
        std::fs::read_to_string(crate::paths::removed_default_routines_path())
            .unwrap()
            .contains(&slug),
        "precondition: the tombstone must be recorded before svc_create"
    );

    let store = new_store();
    let mut req = valid_create_request();
    req.title = title.into();
    with_working_crontab(|| {
        svc_create(&store, req).unwrap();
    });

    let tombstones =
        std::fs::read_to_string(crate::paths::removed_default_routines_path()).unwrap_or_default();
    assert!(
        !tombstones.contains(&slug),
        "svc_create must clear the tombstone for a matching default title"
    );
}
