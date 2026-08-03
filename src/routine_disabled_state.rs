//! Read/write support for the tracked `disabled.json` routine marker.

use chrono::{SecondsFormat, Utc};

use crate::utils::atomic::atomic_write;

/// Resolve enabled state from `disabled.json` first, then legacy `routine.toml`.
pub(crate) fn read_enabled_state(
    base: &std::path::Path,
    dir_name: &str,
    legacy_enabled: Option<bool>,
) -> bool {
    if disabled_marker_path(base, dir_name).exists() {
        false
    } else {
        legacy_enabled.unwrap_or(true)
    }
}

/// Read the optional human-provided disable reason from `disabled.json` metadata.
pub(crate) fn read_disabled_reason(base: &std::path::Path, dir_name: &str) -> Option<String> {
    let text = std::fs::read_to_string(disabled_marker_path(base, dir_name)).ok()?;
    let value = serde_json::from_str::<serde_json::Value>(&text).ok()?;
    value
        .get("reason")
        .and_then(serde_json::Value::as_str)
        .and_then(normalize_reason)
}

/// Persist the source-of-truth disabled marker for a routine.
pub(crate) fn write_disabled_state(
    rel_dir: &str,
    enabled: bool,
    reason: Option<&str>,
) -> std::io::Result<()> {
    let path = crate::paths::routine_disabled_json_path(rel_dir);
    if enabled {
        if path.exists() {
            std::fs::remove_file(path)?;
        }
        return Ok(());
    }
    let reason = reason.and_then(normalize_reason);
    let mut marker = serde_json::json!({
        "version": 1,
        "disabled_at": Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true),
        "disabled_by_machine": crate::machine::current_machine(),
        "disabled_by_user": current_user(),
        "source": "daemon",
    });
    if let Some(reason) = reason {
        marker["reason"] = serde_json::Value::String(reason);
    }
    let text = marker.to_string();
    atomic_write(&path, text.as_bytes())?;
    Ok(())
}

/// Resolve the disabled marker path under any scan base.
fn disabled_marker_path(base: &std::path::Path, dir_name: &str) -> std::path::PathBuf {
    base.join(dir_name).join("disabled.json")
}

/// Trim a reason and collapse blank values to absent.
fn normalize_reason(reason: &str) -> Option<String> {
    let reason = reason.trim();
    (!reason.is_empty()).then(|| reason.to_string())
}

/// Best-effort current OS user for audit metadata.
fn current_user() -> Option<String> {
    ["USER", "USERNAME"]
        .into_iter()
        .find_map(|key| std::env::var(key).ok())
        .filter(|value| !value.trim().is_empty())
}

#[cfg(test)]
#[path = "routine_disabled_state_tests.rs"]
mod routine_disabled_state_tests;
