#![allow(clippy::missing_docs_in_private_items)]

use super::*;

/// RAII guard: redirect config paths (`MOADIM_HOME_OVERRIDE`) to a fresh temp dir for the
/// duration of a test, restoring the previous value and removing the temp dir on drop.
struct HomeOverrideGuard {
    dir: std::path::PathBuf,
    previous: Option<std::ffi::OsString>,
}

impl HomeOverrideGuard {
    fn new() -> Self {
        let dir = std::env::temp_dir().join(format!("moadim-runs-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("create temp home");
        let previous = std::env::var_os("MOADIM_HOME_OVERRIDE");
        // SAFETY: single-threaded test execution (RUST_TEST_THREADS=1).
        unsafe { std::env::set_var("MOADIM_HOME_OVERRIDE", &dir) }
        Self { dir, previous }
    }
}

impl Drop for HomeOverrideGuard {
    fn drop(&mut self) {
        // SAFETY: single-threaded test execution.
        unsafe {
            match self.previous.take() {
                Some(value) => std::env::set_var("MOADIM_HOME_OVERRIDE", value),
                None => std::env::remove_var("MOADIM_HOME_OVERRIDE"),
            }
        }
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn make_job(id: &str, handler: &str) -> CronJob {
    CronJob {
        id: id.to_string(),
        schedule: "@daily".to_string(),
        handler: handler.to_string(),
        metadata: serde_json::Value::Null,
        machines: vec![],
        enabled: true,
        source: "managed".to_string(),
        created_at: 0,
        updated_at: 0,
        last_manual_trigger_at: None,
    }
}

fn make_record(job_id: &str, trigger: RunTrigger) -> RunRecord {
    RunRecord {
        id: uuid::Uuid::new_v4().to_string(),
        job_id: job_id.to_string(),
        started_at: 100,
        finished_at: 101,
        duration_ms: 1_000,
        exit_code: Some(0),
        trigger,
        stdout: "out".to_string(),
        stderr: String::new(),
    }
}

#[test]
fn truncate_output_passes_short_text_through() {
    assert_eq!(truncate_output("hello"), "hello");
    assert_eq!(truncate_output(""), "");
}

#[test]
fn truncate_output_caps_long_text() {
    let long = "a".repeat(MAX_OUTPUT_BYTES + 500);
    let truncated = truncate_output(&long);
    assert_eq!(truncated.len(), MAX_OUTPUT_BYTES);
}

#[test]
fn truncate_output_respects_utf8_char_boundary() {
    // Multi-byte characters padded so the byte cutoff would otherwise land mid-character.
    let long = format!("{}{}", "a".repeat(MAX_OUTPUT_BYTES - 1), "é".repeat(10));
    let truncated = truncate_output(&long);
    assert!(truncated.len() <= MAX_OUTPUT_BYTES);
    // Must still be valid UTF-8 (would panic on an invalid slice boundary otherwise).
    assert!(std::str::from_utf8(truncated.as_bytes()).is_ok());
}

#[test]
fn execute_and_capture_handles_missing_handler() {
    let job = make_job("j1", "does-not-exist");
    let missing_path = std::path::PathBuf::from("/nonexistent/moadim-test-handler-xyz");
    let record = execute_and_capture(&job, &missing_path, RunTrigger::Manual);
    assert_eq!(record.job_id, "j1");
    assert_eq!(record.exit_code, None);
    assert_eq!(record.stdout, "");
    assert_eq!(record.stderr, "");
    assert_eq!(record.trigger, RunTrigger::Manual);
}

#[test]
fn execute_and_capture_handles_spawn_failure() {
    let _home = HomeOverrideGuard::new();
    let handlers = crate::paths::handlers_dir();
    std::fs::create_dir_all(&handlers).expect("create handlers dir");
    let handler_path = handlers.join(format!("nonexec-{}", uuid::Uuid::new_v4()));
    std::fs::write(&handler_path, "not executable").expect("write handler stub");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(&handler_path, std::fs::Permissions::from_mode(0o644))
            .expect("strip exec bit");
    }

    let job = make_job("j2", "nonexec");
    let record = execute_and_capture(&job, &handler_path, RunTrigger::Manual);
    assert_eq!(record.exit_code, None);
}

#[cfg(unix)]
#[test]
fn execute_and_capture_runs_existing_handler() {
    let _home = HomeOverrideGuard::new();
    let handlers = crate::paths::handlers_dir();
    std::fs::create_dir_all(&handlers).expect("create handlers dir");
    let handler_path = handlers.join(format!("ok-{}", uuid::Uuid::new_v4()));
    std::fs::write(
        &handler_path,
        "#!/bin/sh\necho hello-stdout\necho hello-stderr >&2\nexit 7\n",
    )
    .expect("write handler script");
    {
        use std::os::unix::fs::PermissionsExt as _;
        let mut perms = std::fs::metadata(&handler_path).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&handler_path, perms).unwrap();
    }

    let job = make_job("j3", "ok");
    let record = execute_and_capture(&job, &handler_path, RunTrigger::Scheduled);
    assert_eq!(record.exit_code, Some(7));
    assert!(record.stdout.contains("hello-stdout"));
    assert!(record.stderr.contains("hello-stderr"));
    assert_eq!(record.trigger, RunTrigger::Scheduled);
    assert!(record.finished_at >= record.started_at);
}

#[test]
fn append_and_load_runs_roundtrip() {
    let _home = HomeOverrideGuard::new();
    let job_id = "roundtrip-job";
    let first = make_record(job_id, RunTrigger::Manual);
    let second = make_record(job_id, RunTrigger::Scheduled);
    append_run(job_id, &first).expect("append first");
    append_run(job_id, &second).expect("append second");

    let loaded = load_runs(job_id).expect("load runs");
    // Most-recent first.
    assert_eq!(loaded.len(), 2);
    assert_eq!(loaded[0].id, second.id);
    assert_eq!(loaded[1].id, first.id);
}

#[test]
fn load_runs_missing_file_returns_empty() {
    let _home = HomeOverrideGuard::new();
    let loaded = load_runs("never-existed").expect("load runs on missing file");
    assert!(loaded.is_empty());
}

#[test]
fn load_runs_skips_unparseable_lines() {
    let _home = HomeOverrideGuard::new();
    let job_id = "corrupt-job";
    let path = crate::paths::job_runs_path(job_id);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let good = make_record(job_id, RunTrigger::Manual);
    let body = format!(
        "not valid json\n{}\n",
        serde_json::to_string(&good).unwrap()
    );
    std::fs::write(&path, body).unwrap();

    let loaded = load_runs(job_id).expect("load runs tolerating a bad line");
    assert_eq!(loaded.len(), 1);
    assert_eq!(loaded[0].id, good.id);
}

#[test]
fn append_run_caps_at_max_runs_per_job() {
    let _home = HomeOverrideGuard::new();
    let job_id = "overflow-job";
    for _ in 0..MAX_RUNS_PER_JOB + 10 {
        let record = make_record(job_id, RunTrigger::Manual);
        append_run(job_id, &record).expect("append run");
    }
    let loaded = load_runs(job_id).expect("load runs");
    assert_eq!(loaded.len(), MAX_RUNS_PER_JOB);
}

#[test]
fn spawn_capture_and_append_persists_a_run_record() {
    let _home = HomeOverrideGuard::new();
    let handlers = crate::paths::handlers_dir();
    std::fs::create_dir_all(&handlers).expect("create handlers dir");
    let handler_name = format!("spawn-ok-{}", uuid::Uuid::new_v4());
    let handler_path = handlers.join(&handler_name);
    std::fs::write(&handler_path, "#!/bin/sh\nexit 0\n").expect("write handler script");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let mut perms = std::fs::metadata(&handler_path).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&handler_path, perms).unwrap();
    }

    let job = make_job("spawn-job", &handler_name);
    spawn_capture_and_append(job.clone(), RunTrigger::Manual)
        .join()
        .expect("capture thread joins");

    let loaded = load_runs(&job.id).expect("load runs after capture");
    assert_eq!(loaded.len(), 1);
    assert_eq!(loaded[0].exit_code, Some(0));
    assert_eq!(loaded[0].trigger, RunTrigger::Manual);
}

#[cfg(unix)]
#[test]
fn append_run_returns_err_when_parent_dir_creation_fails() {
    use std::os::unix::fs::PermissionsExt as _;
    let home = HomeOverrideGuard::new();
    let jobs_dir = home.dir.join(".config").join("moadim").join("jobs");
    std::fs::create_dir_all(&jobs_dir).expect("create jobs dir");
    std::fs::set_permissions(&jobs_dir, std::fs::Permissions::from_mode(0o555))
        .expect("make jobs dir read-only");

    let record = make_record("perm-fail-job", RunTrigger::Manual);
    let result = append_run("perm-fail-job", &record);
    assert!(result.is_err());

    // Restore write permission so the guard's teardown can remove the temp home.
    std::fs::set_permissions(&jobs_dir, std::fs::Permissions::from_mode(0o755))
        .expect("restore jobs dir permissions");
}

#[cfg(unix)]
#[test]
fn capture_and_append_logs_and_swallows_a_persist_failure() {
    use std::os::unix::fs::PermissionsExt as _;
    let home = HomeOverrideGuard::new();
    let jobs_dir = home.dir.join(".config").join("moadim").join("jobs");
    std::fs::create_dir_all(&jobs_dir).expect("create jobs dir");
    std::fs::set_permissions(&jobs_dir, std::fs::Permissions::from_mode(0o555))
        .expect("make jobs dir read-only");

    let job = make_job("perm-fail-job2", "does-not-exist");
    let missing_path = std::path::PathBuf::from("/nonexistent/moadim-test-handler-xyz2");
    // `capture_and_append` has no return value to assert on; this exercises its `append_run`
    // failure branch (logged, not propagated) without panicking.
    capture_and_append(&job, &missing_path, RunTrigger::Manual);

    std::fs::set_permissions(&jobs_dir, std::fs::Permissions::from_mode(0o755))
        .expect("restore jobs dir permissions");
}
