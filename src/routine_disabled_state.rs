//! Read/write support for the tracked `disabled.json` routine marker.

use chrono::{SecondsFormat, Utc};

use crate::utils::atomic::atomic_write;

/// Resolve enabled state from `disabled.json` first, then legacy `routine.toml`.
pub(crate) fn read_enabled_state(
    base: &std::path::Path,
    dir_name: &str,
    legacy_enabled: Option<bool>,
) -> bool {
    if base.join(dir_name).join("disabled.json").exists() {
        false
    } else {
        legacy_enabled.unwrap_or(true)
    }
}

/// Persist the source-of-truth disabled marker for a routine.
pub(crate) fn write_disabled_state(rel_dir: &str, enabled: bool) -> std::io::Result<()> {
    let path = crate::paths::routine_disabled_json_path(rel_dir);
    if enabled {
        if path.exists() {
            std::fs::remove_file(path)?;
        }
        return Ok(());
    }
    let marker = serde_json::json!({
        "version": 1,
        "disabled_at": Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true),
        "disabled_by_machine": crate::machine::current_machine(),
        "disabled_by_user": current_user(),
        "source": "daemon",
    });
    let text = marker.to_string();
    atomic_write(&path, text.as_bytes())?;
    Ok(())
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
