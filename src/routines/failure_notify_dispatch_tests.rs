#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "tests assert filesystem and process fixtures directly"
)]

use super::super::{
    dispatch_command, dispatch_webhook, exit_reason, notify_finished_run, post_webhook,
    FailureNotificationConfig, RunStatus, CURL_BIN_ENV, WEBHOOK_MAX_ATTEMPTS,
};
use super::{temp_dir, EnvGuard};

fn curl_shim(dir: &std::path::Path, code: i32) -> std::path::PathBuf {
    let path = dir.join(format!("curl-{code}.sh"));
    std::fs::write(
        &path,
        format!("#!/bin/sh\nfor arg do target=\"$arg\"; done\ncat > \"$target\"\nprintf err >&2\nexit {code}\n"),
    )
    .unwrap();
    std::process::Command::new("chmod")
        .arg("+x")
        .arg(&path)
        .status()
        .unwrap();
    path
}

/// A `curl` shim that fails on its first `fail_count` invocations (writing
/// nothing) then succeeds and writes `stdin` to the final positional arg.
/// Tracks the number of invocations in `dir/attempts.count`.
fn flaky_curl_shim(dir: &std::path::Path, fail_count: u32) -> std::path::PathBuf {
    let path = dir.join("curl-flaky.sh");
    let counter = dir.join("attempts.count");
    std::fs::write(
        &path,
        format!(
            "#!/bin/sh\nn=$(cat {counter} 2>/dev/null || echo 0)\nn=$((n+1))\necho \"$n\" > {counter}\nfor arg do target=\"$arg\"; done\nif [ \"$n\" -le {fail_count} ]; then\n  printf err >&2\n  exit 1\nfi\ncat > \"$target\"\nexit 0\n",
            counter = counter.display(),
        ),
    )
    .unwrap();
    std::process::Command::new("chmod")
        .arg("+x")
        .arg(&path)
        .status()
        .unwrap();
    path
}

#[test]
fn notify_failure_dispatches_webhook_with_payload() {
    let dir = temp_dir("notify-webhook");
    let sink = dir.join("sink.json");
    let _curl = EnvGuard::set(CURL_BIN_ENV, curl_shim(&dir, 0));
    let mut routine = super::routine(FailureNotificationConfig::default());
    routine.notifications.on_failure_webhook = Some(sink.to_string_lossy().into_owned());
    let workbench = dir.join("run-42");
    std::fs::create_dir_all(&workbench).unwrap();
    std::fs::write(workbench.join("agent.log"), "web tail").unwrap();

    notify_finished_run(
        &routine,
        "run-42",
        &workbench,
        1,
        2,
        RunStatus::Failed,
        Some(7),
    );

    let mut body = String::new();
    for _ in 0..100 {
        body = std::fs::read_to_string(&sink).unwrap_or_default();
        if serde_json::from_str::<serde_json::Value>(&body).is_ok() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let sent: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(sent["routine_id"], routine.id);
    assert_eq!(sent["exit_reason"], "exit_code_7");
    assert_eq!(sent["log_tail"], "web tail");
}

#[test]
fn webhook_spawn_and_nonzero_failures_are_best_effort() {
    post_webhook("/definitely/missing/curl", "https://example.invalid", b"{}");
    let dir = temp_dir("webhook-nonzero");
    post_webhook(
        &curl_shim(&dir, 2).to_string_lossy(),
        "https://example.invalid",
        b"{}",
    );
}

#[test]
fn unknown_exit_reason_is_reported_when_no_exit_code_or_kill_sentinel() {
    let dir = temp_dir("unknown");
    assert_eq!(exit_reason(&dir, None), "unknown");
}

#[test]
fn command_hook_uses_default_shell_when_env_is_absent() {
    let old = std::env::var_os("MOADIM_SH_BIN");
    // SAFETY: the crate test harness runs single-threaded.
    unsafe { std::env::remove_var("MOADIM_SH_BIN") };
    let dir = super::temp_dir("moadim-failure-command-default-shell");
    let sink = dir.join("out");
    let routine = super::routine(FailureNotificationConfig {
        on_failure_command: Some(format!("echo ok > {}", sink.display())),
        on_failure_webhook: None,
    });

    dispatch_command(
        "echo ok > $MOADIM_LOG_PATH",
        &routine,
        "run",
        "exit_code_1",
        sink.to_str().unwrap(),
        1,
        2,
    );
    for _ in 0..50 {
        if sink.exists() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }

    assert_eq!(std::fs::read_to_string(sink).unwrap().trim(), "ok");
    // SAFETY: the crate test harness runs single-threaded.
    unsafe {
        if let Some(value) = old {
            std::env::set_var("MOADIM_SH_BIN", value);
        }
    }
}

#[test]
fn post_webhook_retries_and_delivers_after_transient_failures() {
    let dir = temp_dir("webhook-retry-success");
    let sink = dir.join("sink.json");
    // Fails on every attempt but the last, so delivery only succeeds because
    // of the retry loop.
    let shim = flaky_curl_shim(&dir, WEBHOOK_MAX_ATTEMPTS - 1);

    post_webhook(
        &shim.to_string_lossy(),
        &sink.to_string_lossy(),
        b"{\"ok\":true}",
    );

    assert_eq!(
        std::fs::read_to_string(&sink).unwrap(),
        "{\"ok\":true}",
        "final attempt should have delivered the payload"
    );
    let attempts: u32 = std::fs::read_to_string(dir.join("attempts.count"))
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    assert_eq!(attempts, WEBHOOK_MAX_ATTEMPTS);
}

#[test]
fn post_webhook_gives_up_and_logs_after_exhausting_retries() {
    let dir = temp_dir("webhook-retry-exhausted");
    let sink = dir.join("sink.json");
    // Always fails: even the last attempt is a failure.
    let shim = flaky_curl_shim(&dir, WEBHOOK_MAX_ATTEMPTS + 5);

    post_webhook(&shim.to_string_lossy(), &sink.to_string_lossy(), b"{}");

    assert!(!sink.exists(), "payload should never have been delivered");
    let attempts: u32 = std::fs::read_to_string(dir.join("attempts.count"))
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    assert_eq!(
        attempts, WEBHOOK_MAX_ATTEMPTS,
        "should stop retrying at the bounded attempt limit"
    );
}

#[test]
fn dispatch_webhook_uses_default_curl_binary() {
    let old_curl = std::env::var_os(CURL_BIN_ENV);
    // SAFETY: the crate test harness runs single-threaded.
    unsafe { std::env::remove_var(CURL_BIN_ENV) };
    let dir = temp_dir("default-curl");
    let shim = curl_shim(&dir, 0);
    let path = format!("{}:{}", dir.display(), std::env::var("PATH").unwrap());
    let _path = EnvGuard::set("PATH", path);
    std::fs::copy(&shim, dir.join("curl")).unwrap();
    let sink = dir.join("sink.json");
    let payload = super::super::FailurePayload {
        routine_id: "routine",
        routine_title: "Routine",
        run_id: "run",
        exit_reason: "exit_code_9",
        started_at: 1,
        finished_at: 2,
        log_path: "log",
        log_tail: "tail",
    };

    dispatch_webhook(sink.to_str().unwrap(), &payload);
    for _ in 0..100 {
        if std::fs::read_to_string(&sink)
            .unwrap_or_default()
            .contains("exit_code_9")
        {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(std::fs::read_to_string(sink)
        .unwrap()
        .contains("exit_code_9"));
    // SAFETY: the crate test harness runs single-threaded.
    unsafe {
        if let Some(value) = old_curl {
            std::env::set_var(CURL_BIN_ENV, value);
        }
    }
}
