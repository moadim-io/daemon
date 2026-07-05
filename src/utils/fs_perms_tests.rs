#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use super::create_private_dir_all;

#[cfg(unix)]
#[test]
fn create_private_dir_all_makes_every_component_owner_only() {
    use std::os::unix::fs::PermissionsExt;

    let base = std::env::temp_dir().join(format!("moadim-fsperms-{}", uuid::Uuid::new_v4()));
    let nested = base.join("a").join("b");
    create_private_dir_all(&nested).unwrap();

    for dir in [base.clone(), base.join("a"), nested.clone()] {
        let mode = std::fs::metadata(&dir).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o700, "{dir:?} should be 0700, got {mode:o}");
    }

    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn create_private_dir_all_is_idempotent_on_existing_dir() {
    let base = std::env::temp_dir().join(format!("moadim-fsperms-idem-{}", uuid::Uuid::new_v4()));
    create_private_dir_all(&base).unwrap();
    // A second call on an existing directory succeeds (mirrors create_dir_all).
    create_private_dir_all(&base).unwrap();
    assert!(base.is_dir());
    let _ = std::fs::remove_dir_all(&base);
}
