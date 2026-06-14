#![allow(clippy::missing_docs_in_private_items)]

use super::*;

#[test]
fn current_does_not_panic() {
    let loc = FsLocation::current();
    // At least one field should be populated in a normal env
    assert!(loc.server_root.is_some() || loc.server_exe_dir.is_some());
}

#[test]
fn serializes_to_json_object() {
    let loc = FsLocation {
        server_root: Some("/tmp".into()),
        server_exe_dir: Some("/usr/bin".into()),
    };
    let val = serde_json::to_value(&loc).unwrap();
    assert_eq!(val["server_root"], "/tmp");
    assert_eq!(val["server_exe_dir"], "/usr/bin");
}

#[test]
fn null_fields_serialize_as_null() {
    let loc = FsLocation {
        server_root: None,
        server_exe_dir: None,
    };
    let val = serde_json::to_value(&loc).unwrap();
    assert!(val["server_root"].is_null());
    assert!(val["server_exe_dir"].is_null());
}
