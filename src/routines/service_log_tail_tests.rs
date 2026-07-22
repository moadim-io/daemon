#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use super::*;

fn temp_log_path(tag: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "moadim-log-tail-test-{tag}-{}",
        uuid::Uuid::new_v4()
    ))
}

#[test]
fn read_log_tail_is_lossy_on_invalid_utf8_when_under_the_cap() {
    let path = temp_log_path("small-invalid-utf8");
    // A lone continuation byte (0x80) is never valid UTF-8 on its own, and this file is well
    // under `MAX_LOG_TAIL_BYTES`, so this exercises the non-truncated read path.
    std::fs::write(&path, b"before\xFFafter\n").expect("write temp log");

    let result = read_log_tail(&path);

    std::fs::remove_file(&path).ok();
    let content = result.expect("invalid UTF-8 in a small log must not error the whole read");
    assert!(content.contains("before"));
    assert!(content.contains("after"));
}

#[test]
fn read_log_tail_still_reads_valid_utf8_when_under_the_cap() {
    let path = temp_log_path("small-valid-utf8");
    std::fs::write(&path, b"hello world\n").expect("write temp log");

    let result = read_log_tail(&path);

    std::fs::remove_file(&path).ok();
    assert_eq!(result.expect("valid UTF-8 read"), "hello world\n");
}
