#![allow(clippy::missing_docs_in_private_items, reason = "test helpers")]

struct TempHome;

impl TempHome {
    fn set() -> Self {
        let dir =
            std::env::temp_dir().join(format!("moadim-overlap-policy-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        // SAFETY: this crate's tests serialize environment mutations.
        unsafe { std::env::set_var("MOADIM_HOME_OVERRIDE", dir) };
        Self
    }
}

impl Drop for TempHome {
    fn drop(&mut self) {
        // SAFETY: this crate's tests serialize environment mutations.
        unsafe { std::env::remove_var("MOADIM_HOME_OVERRIDE") };
    }
}

use super::{read_overlap_policy, write_overlap_policy};

#[test]
fn overlap_policy_defaults_false_and_round_trips_true_then_false() {
    let _home = TempHome::set();
    std::fs::create_dir_all(crate::paths::routine_dir("overlap-policy")).unwrap();
    assert!(!read_overlap_policy("overlap-policy").unwrap());

    write_overlap_policy("overlap-policy", true).unwrap();
    assert!(read_overlap_policy("overlap-policy").unwrap());

    write_overlap_policy("overlap-policy", false).unwrap();
    assert!(!read_overlap_policy("overlap-policy").unwrap());

    write_overlap_policy("overlap-policy", false).unwrap();
    assert!(!read_overlap_policy("overlap-policy").unwrap());
}

#[test]
fn overlap_policy_rejects_malformed_and_unsupported_sidecars() {
    let _home = TempHome::set();
    let path = crate::paths::routine_overlap_json_path("broken-overlap-policy");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, "not json").unwrap();
    assert!(read_overlap_policy("broken-overlap-policy").is_err());

    std::fs::write(&path, r#"{"version":2,"allow_overlapping_runs":true}"#).unwrap();
    assert!(read_overlap_policy("broken-overlap-policy").is_err());
}

#[test]
fn overlap_policy_reports_an_unreadable_sidecar() {
    let _home = TempHome::set();
    let path = crate::paths::routine_overlap_json_path("directory-overlap-policy");
    std::fs::create_dir_all(&path).unwrap();
    assert!(read_overlap_policy("directory-overlap-policy").is_err());
}
