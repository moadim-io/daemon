#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use super::*;

#[test]
fn rotate_daemon_log_if_oversized_is_a_no_op_when_file_is_missing() {
    let path = std::env::temp_dir().join(format!("moadim-log-missing-{}", uuid::Uuid::new_v4()));
    // No file at `path`: must not panic or create anything.
    rotate_daemon_log_if_oversized(&path);
    assert!(!path.exists());
}

#[test]
fn rotate_daemon_log_if_oversized_leaves_small_files_in_place() {
    let base = std::env::temp_dir().join(format!("moadim-log-small-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&base).unwrap();
    let path = base.join("daemon.log");
    std::fs::write(&path, b"a few bytes").unwrap();

    rotate_daemon_log_if_oversized(&path);

    assert!(path.exists(), "file under the cap must not be rotated");
    let mut rotated = path.as_os_str().to_os_string();
    rotated.push(".1");
    assert!(!std::path::Path::new(&rotated).exists());
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn rotate_daemon_log_if_oversized_rolls_the_file_past_the_cap() {
    let base = std::env::temp_dir().join(format!("moadim-log-big-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&base).unwrap();
    let path = base.join("daemon.log");
    std::fs::write(&path, vec![b'x'; (MAX_DAEMON_LOG_BYTES + 1) as usize]).unwrap();

    rotate_daemon_log_if_oversized(&path);

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
fn rotate_daemon_log_if_oversized_replaces_a_previous_1_file() {
    let base = std::env::temp_dir().join(format!("moadim-log-replace-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&base).unwrap();
    let path = base.join("daemon.log");
    std::fs::write(&path, vec![b'y'; (MAX_DAEMON_LOG_BYTES + 1) as usize]).unwrap();
    let mut rotated = path.as_os_str().to_os_string();
    rotated.push(".1");
    let rotated = std::path::PathBuf::from(rotated);
    std::fs::write(&rotated, b"stale rotated content").unwrap();

    rotate_daemon_log_if_oversized(&path);

    assert!(rotated.exists());
    assert_eq!(
        std::fs::metadata(&rotated).unwrap().len(),
        MAX_DAEMON_LOG_BYTES + 1,
        "rotation must replace a stale .1 file with the freshly-rolled one"
    );
    let _ = std::fs::remove_dir_all(&base);
}
