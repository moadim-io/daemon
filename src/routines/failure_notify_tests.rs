#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "tests assert fixture setup and command side effects directly"
)]

use super::*;
#[path = "failure_notify_dispatch_tests.rs"]
mod dispatch;
use std::path::PathBuf;
struct EnvGuard {
    key: &'static str,
    old: Option<std::ffi::OsString>,
}
impl EnvGuard {
    fn set(key: &'static str, value: impl AsRef<std::ffi::OsStr>) -> Self {
        let old = std::env::var_os(key);
        // SAFETY: the crate test harness runs single-threaded.
        unsafe { std::env::set_var(key, value) };
        Self { key, old }
    }
}
impl Drop for EnvGuard {
    fn drop(&mut self) {
        // SAFETY: the crate test harness runs single-threaded.
        unsafe {
            if let Some(old) = &self.old {
                std::env::set_var(self.key, old);
            } else {
                std::env::remove_var(self.key);
            }
        }
    }
}

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "moadim-failure-notify-{name}-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn routine(config: FailureNotificationConfig) -> Routine {
    Routine {
        id: "routine-1".into(),
        schedule: "@daily".into(),
        schedules: vec!["@daily".into()],
        title: "Failure Hook Routine".into(),
        agent: "claude".into(),
        model: None,
        prompt: "do work".into(),
        goal: None,
        repositories: vec![],
        machines: vec![],
        enabled: true,
        disabled_reason: None,
        source: "managed".into(),
        created_at: 1,
        updated_at: 1,
        last_manual_trigger_at: None,
        last_scheduled_trigger_at: None,
        snoozed_until: None,
        skip_runs: None,
        power_saving: false,
        power_saving_exempt: false,
        consecutive_failures: 0,
        auto_disabled_reason: None,
        ttl_secs: None,
        max_runtime_secs: None,
        failure_threshold: None,
        notifications: config,
        tags: vec![],
        env: std::collections::HashMap::new(),
    }
}

#[test]
fn no_config_is_empty_and_success_is_ignored() {
    let dir = temp_dir("noop");
    let routine = routine(FailureNotificationConfig::default());
    notify_finished_run(&routine, "run-1", &dir, 10, 20, RunStatus::Success, Some(0));
    assert!(FailureNotificationConfig::default().is_empty());
}

#[test]
fn command_hook_receives_failure_context_env() {
    let dir = temp_dir("command");
    std::fs::write(dir.join("agent.log"), "first\nsecond\n").unwrap();
    let out = dir.join("hook.out");
    let _shell = EnvGuard::set("MOADIM_SH_BIN", "/bin/sh");
    let command = format!(
        "printf '%s|%s|%s|%s|%s|%s' \"$MOADIM_ROUTINE\" \"$MOADIM_ROUTINE_TITLE\" \"$MOADIM_RUN_ID\" \"$MOADIM_EXIT_REASON\" \"$MOADIM_STARTED_AT\" \"$MOADIM_FINISHED_AT\" > {}",
        out.display()
    );
    let routine = routine(FailureNotificationConfig {
        on_failure_command: Some(command),
        on_failure_webhook: None,
    });

    notify_finished_run(
        &routine,
        "slug-10_99",
        &dir,
        10,
        20,
        RunStatus::Failed,
        Some(7),
    );
    for _ in 0..50 {
        if out.exists() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert_eq!(
        std::fs::read_to_string(out).unwrap(),
        "routine-1|Failure Hook Routine|slug-10_99|exit_code_7|10|20"
    );
}

#[test]
fn global_config_is_used_when_routine_has_no_override() {
    let home = temp_dir("global-home");
    let _home = EnvGuard::set("MOADIM_HOME_OVERRIDE", &home);
    std::fs::create_dir_all(crate::paths::config_dir()).unwrap();
    std::fs::write(
        crate::paths::notifications_toml_path(),
        "[notifications]\non_failure_command = \"echo global\"\n",
    )
    .unwrap();
    assert_eq!(
        global_config().on_failure_command.as_deref(),
        Some("echo global")
    );
}

#[test]
fn flat_global_config_aliases_are_accepted() {
    let home = temp_dir("flat-home");
    let _home = EnvGuard::set("MOADIM_HOME_OVERRIDE", &home);
    std::fs::create_dir_all(crate::paths::config_dir()).unwrap();
    std::fs::write(
        crate::paths::notifications_toml_path(),
        "on_failure_webhook = \"https://example.invalid/hook\"\n",
    )
    .unwrap();
    assert_eq!(
        global_config().on_failure_webhook.as_deref(),
        Some("https://example.invalid/hook")
    );
}

#[test]
fn timeout_reason_reads_watchdog_killed_sentinel() {
    let dir = temp_dir("timeout");
    std::fs::write(dir.join("exit_code"), "killed\n").unwrap();
    assert_eq!(exit_reason(&dir, None), "timeout");
    assert_eq!(exit_reason(&dir, Some(3)), "exit_code_3");
}

#[test]
fn webhook_post_uses_curl_shim_and_stdin_json() {
    let dir = temp_dir("webhook");
    let shim = dir.join("curl-shim.sh");
    let out = dir.join("webhook.json");
    std::fs::write(
        &shim,
        format!("#!/bin/sh\ncat > {}\nexit 0\n", out.display()),
    )
    .unwrap();
    let _ = std::process::Command::new("chmod")
        .arg("+x")
        .arg(&shim)
        .status();
    let payload = FailurePayload {
        routine_id: "r1",
        routine_title: "Routine",
        run_id: "run1",
        started_at: 1,
        finished_at: 2,
        exit_reason: "unknown",
        log_path: "/tmp/agent.log",
        log_tail: "tail",
    };
    let body = serde_json::to_vec(&payload).unwrap();
    post_webhook(
        shim.to_str().unwrap(),
        "https://example.invalid/hook",
        &body,
    );
    let sent: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(out).unwrap()).unwrap();
    assert_eq!(sent["routine_id"], "r1");
    assert_eq!(sent["log_tail"], "tail");
    let path = dir.join("agent.log");
    std::fs::write(&path, "x".repeat(LOG_TAIL_BYTES + 5)).unwrap();
    assert_eq!(read_tail(&path).len(), LOG_TAIL_BYTES);
}
