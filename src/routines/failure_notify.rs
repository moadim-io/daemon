//! Failure notification hook dispatch for completed routine runs.
#![allow(
    clippy::missing_docs_in_private_items,
    reason = "private helpers have focused tests"
)]

use std::io::Write as _;
use std::path::Path;

use super::{FailureNotificationConfig, Routine, RunStatus};
const CURL_BIN_ENV: &str = "MOADIM_NOTIFICATION_CURL_BIN";
const LOG_TAIL_BYTES: usize = 8 * 1024;

#[derive(serde::Deserialize)]
struct GlobalNotificationsToml {
    notifications: Option<FailureNotificationConfig>,
    on_failure_command: Option<String>,
    on_failure_webhook: Option<String>,
}
#[derive(serde::Serialize)]
struct FailurePayload<'a> {
    /// Routine UUID.
    routine_id: &'a str,
    /// Routine title.
    routine_title: &'a str,
    /// Workbench/run directory name.
    run_id: &'a str,
    /// Unix seconds when the run started.
    started_at: u64,
    /// Unix seconds when the run finished.
    finished_at: u64,
    /// Exit reason, e.g. `exit_code_1` or `timeout`.
    exit_reason: &'a str,
    /// Path to the run's `agent.log`.
    log_path: &'a str,
    /// Bounded tail of the run's `agent.log`.
    log_tail: &'a str,
}

/// Notify configured hooks for a non-success run. No-op when no hooks are configured.
pub(crate) fn notify_finished_run(
    routine: &Routine,
    workbench_name: &str,
    workbench_path: &Path,
    started_at: u64,
    finished_at: u64,
    status: RunStatus,
    exit_code: Option<i32>,
) {
    if status == RunStatus::Success {
        return;
    }
    let config = effective_config(routine);
    if config.is_empty() {
        return;
    }
    let log_path = workbench_path.join("agent.log");
    let log_path_text = log_path.display().to_string();
    let log_tail = read_tail(&log_path);
    let exit_reason = exit_reason(workbench_path, exit_code);
    if let Some(command) = config.on_failure_command {
        dispatch_command(
            &command,
            routine,
            workbench_name,
            &exit_reason,
            &log_path_text,
            started_at,
            finished_at,
        );
    }
    if let Some(webhook) = config.on_failure_webhook {
        let payload = FailurePayload {
            routine_id: &routine.id,
            routine_title: &routine.title,
            run_id: workbench_name,
            started_at,
            finished_at,
            exit_reason: &exit_reason,
            log_path: &log_path_text,
            log_tail: &log_tail,
        };
        dispatch_webhook(&webhook, &payload);
    }
}

fn effective_config(routine: &Routine) -> FailureNotificationConfig {
    if !routine.notifications.is_empty() {
        return routine.notifications.clone();
    }
    global_config()
}

fn global_config() -> FailureNotificationConfig {
    std::fs::read_to_string(crate::paths::notifications_toml_path())
        .ok()
        .and_then(|text| toml::from_str::<GlobalNotificationsToml>(&text).ok())
        .map(|parsed| {
            parsed.notifications.unwrap_or(FailureNotificationConfig {
                on_failure_command: parsed.on_failure_command,
                on_failure_webhook: parsed.on_failure_webhook,
            })
        })
        .unwrap_or_default()
}

fn exit_reason(workbench_path: &Path, exit_code: Option<i32>) -> String {
    if let Some(code) = exit_code {
        return format!("exit_code_{code}");
    }
    if std::fs::read_to_string(workbench_path.join("exit_code"))
        .is_ok_and(|text| text.trim() == "killed")
    {
        return "timeout".to_string();
    }
    "unknown".to_string()
}

fn read_tail(path: &Path) -> String {
    let bytes = std::fs::read(path).unwrap_or_default();
    let start = bytes.len().saturating_sub(LOG_TAIL_BYTES);
    String::from_utf8_lossy(&bytes[start..]).into_owned()
}

#[allow(
    clippy::too_many_arguments,
    reason = "env contract is explicit and mirrors docs/tests"
)]
fn dispatch_command(
    command: &str,
    routine: &Routine,
    run_id: &str,
    exit_reason: &str,
    log_path: &str,
    started_at: u64,
    finished_at: u64,
) {
    let shell = std::env::var("MOADIM_SH_BIN").unwrap_or_else(|_| "sh".to_string());
    let mut child = std::process::Command::new(shell);
    child
        .arg("-lc")
        .arg(command)
        .env("MOADIM_ROUTINE", &routine.id)
        .env("MOADIM_ROUTINE_TITLE", &routine.title)
        .env("MOADIM_RUN_ID", run_id)
        .env("MOADIM_EXIT_REASON", exit_reason)
        .env("MOADIM_LOG_PATH", log_path)
        .env("MOADIM_STARTED_AT", started_at.to_string())
        .env("MOADIM_FINISHED_AT", finished_at.to_string());
    crate::utils::process::spawn_and_reap(child, "failure notification command");
}

#[allow(clippy::expect_used, reason = "payload serialization is infallible")]
fn dispatch_webhook(webhook: &str, payload: &FailurePayload<'_>) {
    let body = serde_json::to_vec(payload).expect("failure payload serializes");
    let curl_bin = std::env::var(CURL_BIN_ENV).unwrap_or_else(|_| "curl".to_string());
    let webhook = webhook.to_string();
    std::thread::spawn(move || post_webhook(&curl_bin, &webhook, &body));
}

#[allow(clippy::expect_used, reason = "spawned curl child should wait")]
fn post_webhook(curl_bin: &str, webhook: &str, body: &[u8]) {
    let mut child = match std::process::Command::new(curl_bin)
        .arg("-fsS")
        .arg("--max-time")
        .arg("10")
        .arg("-X")
        .arg("POST")
        .arg("-H")
        .arg("content-type: application/json")
        .arg("--data-binary")
        .arg("@-")
        .arg(webhook)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .spawn()
    {
        Ok(child) => child,
        Err(err) => {
            log::warn!("failure notification webhook: failed to spawn curl: {err}");
            return;
        }
    };
    let _ = child.stdin.take().map(|mut stdin| stdin.write_all(body));
    let out = child.wait_with_output().expect("curl child exits");
    if !out.status.success() {
        log::warn!(
            "failure notification webhook failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
}

#[cfg(test)]
#[path = "failure_notify_tests.rs"]
mod failure_notify_tests;
