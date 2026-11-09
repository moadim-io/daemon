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
    notifications: Option<NotificationsTable>,
    on_failure_command: Option<String>,
    on_failure_webhook: Option<String>,
    public_base_url: Option<String>,
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
    /// Machine name of the host the run failed on.
    machine: &'a str,
    /// Numeric exit code, or `null` for a timeout / unknown outcome.
    exit_code: Option<i32>,
    /// Run Detail page URL in the web UI.
    run_url: &'a str,
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
    let context = RunContext::new(&routine.id, workbench_name, exit_code);
    if let Some(command) = config.on_failure_command {
        dispatch_command(
            &command,
            routine,
            workbench_name,
            &exit_reason,
            &log_path_text,
            started_at,
            finished_at,
            &context,
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
            machine: &context.machine,
            exit_code: context.exit_code,
            run_url: &context.run_url,
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
    read_global_toml()
        .map(|parsed| {
            parsed.notifications.map_or(
                FailureNotificationConfig {
                    on_failure_command: parsed.on_failure_command,
                    on_failure_webhook: parsed.on_failure_webhook,
                },
                |table| table.hooks,
            )
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
    context: &RunContext,
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
        .env("MOADIM_FINISHED_AT", finished_at.to_string())
        .env("MOADIM_MACHINE", &context.machine)
        .env("MOADIM_RUN_URL", &context.run_url);
    match context.exit_code {
        Some(code) => child.env("MOADIM_EXIT_CODE", code.to_string()),
        None => child.env_remove("MOADIM_EXIT_CODE"),
    };
    crate::utils::process::spawn_and_reap(child, "failure notification command");
}

#[allow(clippy::expect_used, reason = "payload serialization is infallible")]
fn dispatch_webhook(webhook: &str, payload: &FailurePayload<'_>) {
    let body = serde_json::to_vec(payload).expect("failure payload serializes");
    let curl_bin = std::env::var(CURL_BIN_ENV).unwrap_or_else(|_| "curl".to_string());
    let webhook = webhook.to_string();
    std::thread::spawn(move || post_webhook(&curl_bin, &webhook, &body));
}

include!("failure_notify_webhook_retry.rs");
include!("failure_notify_context.rs");

#[cfg(test)]
#[path = "failure_notify_tests.rs"]
mod failure_notify_tests;
