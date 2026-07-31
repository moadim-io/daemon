
#[test]
fn read_log_tail_errors_when_file_is_missing() {
    // The very first thing `read_log_tail` does is `std::fs::metadata(path)?`; a workbench
    // whose `agent.log` was removed out from under it (e.g. a racing cleanup sweep) must
    // surface an `io::Error` here instead of panicking.
    let dir = std::env::temp_dir().join(format!("moadim-logtail-missing-{}", uuid::Uuid::new_v4()));
    let path = dir.join("agent.log");

    assert!(read_log_tail(&path).is_err());
}

#[test]
fn read_log_tail_returns_whole_file_under_the_cap() {
    let dir = std::env::temp_dir().join(format!("moadim-logtail-small-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("agent.log");
    std::fs::write(&path, "short log\n").unwrap();

    assert_eq!(read_log_tail(&path).unwrap(), "short log\n");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn read_log_tail_truncates_and_notes_omitted_bytes() {
    let dir = std::env::temp_dir().join(format!("moadim-logtail-big-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("agent.log");
    // 3 bytes over the cap, so the tail drops exactly the first 3 leading "a"s.
    let mut content = "a".repeat(MAX_LOG_TAIL_BYTES as usize);
    content.push_str("END");
    std::fs::write(&path, &content).unwrap();

    let tail = read_log_tail(&path).unwrap();

    assert_eq!(
        tail,
        format!(
            "... [3 bytes omitted; showing the last {} bytes] ...\n{}END",
            MAX_LOG_TAIL_BYTES,
            "a".repeat(MAX_LOG_TAIL_BYTES as usize - 3),
        )
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn read_log_tail_snaps_to_a_utf8_char_boundary() {
    let dir = std::env::temp_dir().join(format!("moadim-logtail-utf8-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("agent.log");
    // The file is exactly 1 byte over the cap, and its very first character is the 2-byte
    // UTF-8 "é" straddling that 1-byte seek point. A naive byte-offset seek would land on
    // é's stray continuation byte; the fix must skip it instead of emitting invalid UTF-8.
    let content = format!("é{}", "a".repeat(MAX_LOG_TAIL_BYTES as usize - 1));
    std::fs::write(&path, &content).unwrap();

    let tail = read_log_tail(&path).unwrap();

    assert_eq!(
        tail,
        format!(
            "... [1 bytes omitted; showing the last {} bytes] ...\n{}",
            MAX_LOG_TAIL_BYTES,
            "a".repeat(MAX_LOG_TAIL_BYTES as usize - 1),
        )
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn read_log_tail_snaps_to_the_next_line_start() {
    let dir = std::env::temp_dir().join(format!("moadim-logtail-line-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("agent.log");
    // The seek point lands 3 bytes into the word "PARTIAL" (all ASCII, so no UTF-8 boundary
    // issue), which without a line-aware snap would leak "TIAL" as a mangled first line. The
    // fix must also skip past the following '\n' so the kept tail starts on a clean line.
    let mut content = "PAR".to_string();
    content.push_str("TIAL\n");
    content.push_str(&"a".repeat(MAX_LOG_TAIL_BYTES as usize - 5));
    std::fs::write(&path, &content).unwrap();

    let tail = read_log_tail(&path).unwrap();

    assert_eq!(
        tail,
        format!(
            "... [3 bytes omitted; showing the last {} bytes] ...\n{}",
            MAX_LOG_TAIL_BYTES,
            "a".repeat(MAX_LOG_TAIL_BYTES as usize - 5),
        )
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn svc_logs_reads_through_the_size_cap() {
    let _home = TempHome::set();
    // End-to-end: svc_logs must go through read_log_tail, not a raw read_to_string, so an
    // oversized agent.log doesn't get served in full.
    let title = "Svc Logs Big ZZQ";
    let slug = slugify(title);
    let store = new_store();
    store.lock().unwrap().insert(
        "logs-big-id".into(),
        make_routine("logs-big-id", title, 1, 1),
    );

    let workbenches = crate::paths::workbenches_dir();
    let dir = workbenches.join(format!("{slug}-1"));
    std::fs::create_dir_all(&dir).unwrap();
    let big = "x".repeat(MAX_LOG_TAIL_BYTES as usize + 10);
    std::fs::write(dir.join("agent.log"), &big).unwrap();

    let logs = svc_logs(&store, "logs-big-id").unwrap();
    assert_ne!(
        logs.content, big,
        "an oversized log must not be served in full"
    );
    assert!(logs.content.contains("10 bytes omitted"), "got: {logs:?}");
    assert_eq!(logs.total_bytes, big.len() as u64);
    assert!(logs.truncated);
}
