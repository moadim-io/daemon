use super::*;

fn temp_log_path(label: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "moadim-log-cap-{label}-{}.log",
        uuid::Uuid::new_v4()
    ))
}

#[test]
fn cap_agent_log_is_noop_for_a_missing_file() {
    let path = temp_log_path("missing");
    let _ = std::fs::remove_file(&path);
    assert!(!cap_agent_log_if_oversized(&path).unwrap());
}

#[test]
fn cap_agent_log_is_noop_within_budget() {
    let path = temp_log_path("within-budget");
    std::fs::write(&path, "hello agent\n").unwrap();
    assert!(!cap_agent_log_if_oversized(&path).unwrap());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "hello agent\n");
    std::fs::remove_file(&path).unwrap();
}

#[test]
fn cap_agent_log_to_truncates_an_oversized_file_to_its_tail() {
    let path = temp_log_path("oversized");
    std::fs::write(&path, "0123456789ABCDEFGHIJ").unwrap(); // 20 bytes
    let capped = cap_agent_log_to(&path, 10).unwrap();
    assert!(capped);

    let contents = std::fs::read_to_string(&path).unwrap();
    assert!(
        contents.ends_with("ABCDEFGHIJ"),
        "expected the last 10 bytes to survive, got {contents:?}"
    );
    assert!(
        contents.starts_with("... [10 bytes truncated; agent.log capped at 10 bytes] ..."),
        "expected a truncation marker, got {contents:?}"
    );
    std::fs::remove_file(&path).unwrap();
}

#[test]
fn cap_agent_log_to_is_noop_exactly_at_the_cap() {
    let path = temp_log_path("exact-cap");
    std::fs::write(&path, "0123456789").unwrap(); // 10 bytes
    assert!(!cap_agent_log_to(&path, 10).unwrap());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "0123456789");
    std::fs::remove_file(&path).unwrap();
}

#[test]
fn cap_agent_log_or_warn_does_not_panic_on_a_missing_file() {
    let path = temp_log_path("warn-missing");
    let _ = std::fs::remove_file(&path);
    cap_agent_log_or_warn(&path);
}

/// `metadata()` on a path nested under a regular file (not a directory) fails with `NotADirectory`
/// rather than `NotFound`, exercising [`cap_agent_log_to`]'s non-`NotFound`-error passthrough.
#[test]
fn cap_agent_log_propagates_a_non_not_found_metadata_error() {
    let parent = temp_log_path("not-a-dir-parent");
    std::fs::write(&parent, "not a directory").unwrap();
    let path = parent.join("agent.log");

    assert!(cap_agent_log_if_oversized(&path).is_err());
    cap_agent_log_or_warn(&path); // must log and swallow the same error, not panic
    std::fs::remove_file(&parent).unwrap();
}
