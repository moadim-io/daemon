#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use super::*;

#[test]
fn rotate_daemon_log_if_due_is_a_no_op_when_file_is_missing() {
    let path = std::env::temp_dir().join(format!("moadim-log-missing-{}", uuid::Uuid::new_v4()));
    // No file at `path`: must not panic or create anything.
    rotate_daemon_log_if_due(&path);
    assert!(!path.exists());
}

#[test]
fn rotate_daemon_log_if_due_leaves_small_files_in_place() {
    let base = std::env::temp_dir().join(format!("moadim-log-small-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&base).unwrap();
    let path = base.join("daemon.log");
    std::fs::write(&path, b"a few bytes").unwrap();

    rotate_daemon_log_if_due(&path);

    assert!(path.exists(), "file under the cap must not be rotated");
    let mut rotated = path.as_os_str().to_os_string();
    rotated.push(".1");
    assert!(!std::path::Path::new(&rotated).exists());
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn rotate_daemon_log_if_due_rolls_the_file_past_the_cap() {
    let base = std::env::temp_dir().join(format!("moadim-log-big-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&base).unwrap();
    let path = base.join("daemon.log");
    std::fs::write(&path, vec![b'x'; (DAEMON_LOG_MAX_BYTES + 1) as usize]).unwrap();

    rotate_daemon_log_if_due(&path);

    assert!(
        !path.exists(),
        "the oversized file must be moved out of the way"
    );
    let mut rotated = path.as_os_str().to_os_string();
    rotated.push(".1");
    assert!(
        std::path::Path::new(&rotated).exists(),
        "the oversized file must land at the .1 sibling"
    );
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn rotate_daemon_log_if_due_replaces_a_previous_1_file() {
    let base = std::env::temp_dir().join(format!("moadim-log-replace-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&base).unwrap();
    let path = base.join("daemon.log");
    std::fs::write(&path, vec![b'y'; (DAEMON_LOG_MAX_BYTES + 1) as usize]).unwrap();
    let mut rotated = path.as_os_str().to_os_string();
    rotated.push(".1");
    let rotated = std::path::PathBuf::from(rotated);
    std::fs::write(&rotated, b"stale rotated content").unwrap();

    rotate_daemon_log_if_due(&path);

    assert!(rotated.exists());
    assert_eq!(
        std::fs::metadata(&rotated).unwrap().len(),
        DAEMON_LOG_MAX_BYTES + 1,
        "rotation must replace a stale .1 file with the freshly-rolled one"
    );
    let _ = std::fs::remove_dir_all(&base);
}

// `log_rotation_is_due` is the pure trigger predicate `rotate_daemon_log_if_due` reads a real
// file's size/age into; it is tested directly here with injected values since a real file's birth
// time can't be back-dated portably to exercise the "stale" branch end-to-end (#1157).

#[test]
fn log_rotation_is_due_is_false_for_a_small_fresh_log() {
    assert!(!log_rotation_is_due(1024, Duration::from_secs(60)));
}

#[test]
fn log_rotation_is_due_is_true_when_oversized_but_fresh() {
    assert!(log_rotation_is_due(
        DAEMON_LOG_MAX_BYTES + 1,
        Duration::from_secs(0)
    ));
}

#[test]
fn log_rotation_is_due_is_true_when_small_but_stale() {
    // The whole point of #1157: a long-lived daemon whose log never crosses the size cap must
    // still rotate once it's been more than a day since the current segment was created.
    assert!(log_rotation_is_due(
        1024,
        DAEMON_LOG_MAX_AGE + Duration::from_secs(1)
    ));
}

#[test]
fn log_rotation_is_due_is_false_right_at_the_age_boundary() {
    assert!(!log_rotation_is_due(1024, DAEMON_LOG_MAX_AGE));
}
