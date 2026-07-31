
#[test]
fn rotate_run_history_if_oversized_rolls_the_file_past_the_cap() {
    let base = std::env::temp_dir().join(format!("moadim-runs-big-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&base).unwrap();
    let path = base.join("runs.log");
    std::fs::write(&path, vec![b'x'; (RUN_HISTORY_MAX_BYTES + 1) as usize]).unwrap();

    rotate_run_history_if_oversized(&path);

    assert!(
        !path.exists(),
        "the oversized file must be moved out of the way"
    );
    assert!(
        path.with_extension("log.1").exists(),
        "the oversized file must land at the .1 sibling"
    );
    let _ = std::fs::remove_dir_all(&base);
}

/// `rotated_path`'s `std::fs::write` can fail (permissions, a race) even though the oversized
/// source file's own `metadata()`/`read()` just succeeded — e.g. `rotated_path` itself turns out to
/// be a directory. `rotate_run_history_if_oversized` must leave the source file in place rather than
/// remove it, since removal is gated on the write actually succeeding (mirrors the directory-target
/// technique `cap_agent_log_propagates_an_open_error_for_a_directory` uses for the same class of
/// error in `cleanup::log_cap`).
#[test]
fn rotate_run_history_if_oversized_keeps_the_source_when_the_rotated_write_fails() {
    let base = std::env::temp_dir().join(format!("moadim-runs-writefail-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&base).unwrap();
    let path = base.join("runs.log");
    std::fs::write(&path, vec![b'z'; (RUN_HISTORY_MAX_BYTES + 1) as usize]).unwrap();
    let rotated = path.with_extension("log.1");
    std::fs::create_dir(&rotated).unwrap(); // a directory can't be `fs::write`-n over

    rotate_run_history_if_oversized(&path);

    assert!(
        path.exists(),
        "the source file must survive a failed rotation write"
    );
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn rotate_run_history_if_oversized_merges_with_a_previous_1_file() {
    let base = std::env::temp_dir().join(format!("moadim-runs-merge-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&base).unwrap();
    let path = base.join("runs.log");
    std::fs::write(&path, vec![b'y'; (RUN_HISTORY_MAX_BYTES + 1) as usize]).unwrap();
    let rotated = path.with_extension("log.1");
    std::fs::write(&rotated, b"stale rotated content").unwrap();

    rotate_run_history_if_oversized(&path);

    assert!(rotated.exists());
    assert!(
        !path.exists(),
        "the source file is removed once its content is folded into .1"
    );
    assert_eq!(
        std::fs::metadata(&rotated).unwrap().len(),
        "stale rotated content".len() as u64 + RUN_HISTORY_MAX_BYTES + 1,
        "rotation must preserve a stale .1 file's content, not overwrite it (#1277)"
    );
    let _ = std::fs::remove_dir_all(&base);
}

#[allow(
    clippy::missing_docs_in_private_items,
    reason = "split-out module keeps the file under the linecheck limit"
)]
#[path = "append_persisted_run_rotates_an_oversized_log_before_appending_tests.rs"]
mod append_persisted_run_rotates_an_oversized_log_before_appending_tests;
