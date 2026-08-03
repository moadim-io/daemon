//! Higher-level routine data-plane actions that need response parsing before printing.

use serde_json::Value;

use super::routine_path;

/// Flip a single routine's `enabled` flag via `PATCH /routines/{routine}`.
pub(super) fn set_routine_enabled(
    routine: &str,
    enabled: bool,
    reason: Option<String>,
    json: bool,
) -> i32 {
    let body = enabled_patch_body(enabled, reason);
    match crate::cli::http_request_json("PATCH", &routine_path(routine), Some(&body)) {
        Ok((status, resp)) if (200..300).contains(&status) => {
            report_enabled(routine, enabled, &resp, json);
            0
        }
        Ok((status, resp)) => report_http_error(status, &resp),
        Err(_) => not_running(),
    }
}

/// Build the routine enablement PATCH body.
fn enabled_patch_body(enabled: bool, reason: Option<String>) -> String {
    if enabled {
        serde_json::json!({ "enabled": true })
    } else {
        let mut body = serde_json::json!({ "enabled": false });
        if let Some(reason) = reason {
            body["disabled_reason"] = serde_json::Value::String(reason);
        }
        body
    }
    .to_string()
}

/// Report the outcome of an enable/disable request.
fn report_enabled(routine: &str, requested: bool, resp: &str, json: bool) {
    let parsed = serde_json::from_str::<Value>(resp).ok();
    let id = parsed
        .as_ref()
        .and_then(|value| value.get("id"))
        .and_then(Value::as_str)
        .unwrap_or(routine);
    let state = parsed
        .as_ref()
        .and_then(|value| value.get("enabled"))
        .and_then(Value::as_bool)
        .unwrap_or(requested);
    if json {
        println!("{}", serde_json::json!({ "routine": id, "enabled": state }));
    } else {
        let word = if state { "enabled" } else { "disabled" };
        println!("routine {id} is now {word}");
    }
}

/// Print a server-side error body and return the generic failure exit code.
fn report_http_error(status: u16, resp: &str) -> i32 {
    eprintln!("error: server returned HTTP {status}");
    if !resp.is_empty() {
        eprintln!("{resp}");
    }
    1
}

/// Print the standard liveness failure message and return the not-running exit code.
fn not_running() -> i32 {
    eprintln!("moadim is not running");
    crate::cli::EXIT_NOT_RUNNING
}

#[cfg(test)]
#[path = "commands_routine_actions_tests.rs"]
mod commands_routine_actions_tests;
