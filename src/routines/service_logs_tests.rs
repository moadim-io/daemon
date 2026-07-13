#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use super::*;

use crate::routines::new_store;

struct TempHome(std::path::PathBuf);

impl TempHome {
    fn set() -> Self {
        let dir = std::env::temp_dir().join(format!("moadim-svctest-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("create temp home");
        // SAFETY: single-threaded test execution.
        unsafe {
            std::env::set_var("MOADIM_HOME_OVERRIDE", &dir);
        }
        Self(dir)
    }
}

impl Drop for TempHome {
    fn drop(&mut self) {
        // SAFETY: single-threaded test execution.
        unsafe {
            std::env::remove_var("MOADIM_HOME_OVERRIDE");
        }
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn make_routine(id: &str, title: &str, created_at: u64, updated_at: u64) -> Routine {
    Routine {
        model: None,
        id: id.to_string(),
        schedule: "@daily".to_string(),
        title: title.to_string(),
        agent: "claude".to_string(),
        prompt: "do the thing".to_string(),
        goal: None,
        repositories: vec![],
        machines: vec![crate::machine::current_machine()],
        enabled: true,
        source: "managed".to_string(),
        created_at,
        updated_at,
        last_manual_trigger_at: None,
        last_scheduled_trigger_at: None,
        snoozed_until: None,
        skip_runs: None,
        power_saving: false,
        tags: vec![],
        ttl_secs: None,
        max_runtime_secs: None,
    }
}

#[test]
fn svc_logs_returns_newest_workbench_log() {
    let _home = TempHome::set();
    // Covers the newest-workbench selection inside `svc_logs`: with two valid
    // `{slug}-{ts}` workbench directories, the higher timestamp wins and its
    // `agent.log` contents are returned.
    let title = "Svc Logs Newest ZZZ";
    let slug = slugify(title);
    let store = new_store();
    let mut routine = make_routine("logs-id", title, 1, 1);
    routine.repositories = vec![Repository {
        repository: "https://example.com/r.git".into(),
        branch: None,
    }];
    store.lock().unwrap().insert("logs-id".into(), routine);

    let workbenches = crate::paths::workbenches_dir();
    std::fs::create_dir_all(&workbenches).unwrap();
    let older = workbenches.join(format!("{slug}-1000"));
    let newer = workbenches.join(format!("{slug}-2000"));
    std::fs::create_dir_all(&older).unwrap();
    std::fs::create_dir_all(&newer).unwrap();
    std::fs::write(older.join("agent.log"), "old log contents").unwrap();
    std::fs::write(newer.join("agent.log"), "new log contents").unwrap();

    let logs = svc_logs(&store, "logs-id").unwrap();
    assert_eq!(logs.content, "new log contents");
    assert_eq!(logs.total_bytes, "new log contents".len() as u64);
    assert!(!logs.truncated);
}

#[test]
fn svc_logs_skips_foreign_and_unparseable_workbenches() {
    let _home = TempHome::set();
    // Exercises the read_dir loop body across every arm: a workbench whose name
    // does not parse as `{slug}-{ts}` (parser returns None → skipped), a workbench
    // that parses but belongs to a different routine (`dir_slug != slug` → skipped),
    // and finally this routine's own workbench whose log is returned.
    let title = "Svc Logs Mixed ZZQ";
    let slug = slugify(title);
    let store = new_store();
    let routine = make_routine("logs-mixed-id", title, 1, 1);
    store
        .lock()
        .unwrap()
        .insert("logs-mixed-id".into(), routine);

    let workbenches = crate::paths::workbenches_dir();
    std::fs::create_dir_all(&workbenches).unwrap();

    // Not a `{slug}-{ts}` directory at all: parse_workbench_name returns None.
    let unparseable = workbenches.join("not-a-workbench-name");
    std::fs::create_dir_all(&unparseable).unwrap();
    std::fs::write(unparseable.join("agent.log"), "ignored").unwrap();

    // A well-formed workbench owned by a *different* routine slug.
    let foreign = workbenches.join("some-other-routine-9999");
    std::fs::create_dir_all(&foreign).unwrap();
    std::fs::write(foreign.join("agent.log"), "foreign log").unwrap();

    // This routine's own workbench.
    let mine = workbenches.join(format!("{slug}-4242"));
    std::fs::create_dir_all(&mine).unwrap();
    std::fs::write(mine.join("agent.log"), "mine log contents").unwrap();

    let logs = svc_logs(&store, "logs-mixed-id").unwrap();
    assert_eq!(logs.content, "mine log contents");
}

#[test]
fn svc_logs_empty_when_workbenches_dir_absent() {
    let _home = TempHome::set();
    // Covers the `read_dir` error path in `svc_logs`: the fresh temp home has no `workbenches`
    // subdirectory, so `std::fs::read_dir` returns Err and the loop is skipped entirely. With no
    // workbench found, the function returns an empty string.
    let title = "Svc Logs No Workbenches ZZQ";
    let store = new_store();
    store.lock().unwrap().insert(
        "logs-empty-id".into(),
        make_routine("logs-empty-id", title, 1, 1),
    );

    assert!(!crate::paths::workbenches_dir().exists());

    let logs = svc_logs(&store, "logs-empty-id").unwrap();
    assert_eq!(logs.content, "");
    assert_eq!(logs.total_bytes, 0);
    assert!(!logs.truncated);
}

#[test]
fn svc_logs_missing_routine_not_found() {
    let _home = TempHome::set();
    assert!(matches!(
        svc_logs(&new_store(), "nope"),
        Err(AppError::NotFound)
    ));
}

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
