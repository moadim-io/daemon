#![allow(
    clippy::wildcard_imports,
    clippy::too_many_arguments,
    clippy::needless_pass_by_value,
    dead_code,
    reason = "split-out module preserves existing code while keeping files under the linecheck limit"
)]
use super::*;

#[test]
fn clear_removed_default_is_best_effort_on_write_failure() {
    // Same best-effort contract on the clear path: the tombstone must already contain the slug
    // (so the read succeeds and `remove` returns `true`), but the follow-up persist write fails
    // because the file itself has been made read-only.
    use std::os::unix::fs::PermissionsExt as _;
    with_redirected_home(|_home| {
        let slug = "some-default";
        record_removed_default(slug);
        assert_eq!(read_removed_defaults().len(), 1);

        let path = removed_default_routines_path();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o400)).unwrap();

        clear_removed_default(slug);

        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
    });
}
