//! Host power-saving detection used by routine launch policy.

#[cfg(target_os = "macos")]
use std::process::Command;

/// Return `true` when the host OS reports a power-saving condition that should defer non-critical
/// routine launches.
///
/// V1 is intentionally conservative and cheap: on macOS, battery power or Low Power Mode activates
/// the policy; other platforms return `false` until they get an explicit detector.
pub(crate) fn is_system_power_saving_active() -> bool {
    if let Some(active) = env_override() {
        return active;
    }
    platform_power_saving_active()
}

/// Parse the optional test/operator override for the system power-saving detector.
fn env_override() -> Option<bool> {
    let value = std::env::var("MOADIM_POWER_SAVING_ACTIVE").ok()?;
    match value.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Some(true),
        "0" | "false" | "no" | "off" => Some(false),
        _ => None,
    }
}

/// Platform-specific detector implementation.
#[cfg(target_os = "macos")]
fn platform_power_saving_active() -> bool {
    macos_power_saving_active()
}

/// Platform-specific detector implementation.
#[cfg(not(target_os = "macos"))]
const fn platform_power_saving_active() -> bool {
    false
}

/// macOS detector: treat either battery power or Low Power Mode as a system-saving signal.
#[cfg(target_os = "macos")]
fn macos_power_saving_active() -> bool {
    pmset_battery_power() || pmset_low_power_mode()
}

/// Return whether `pmset -g batt` reports battery power.
#[cfg(target_os = "macos")]
fn pmset_battery_power() -> bool {
    command_stdout(&pmset_bin(), &["-g", "batt"])
        .is_some_and(|stdout| stdout.contains("'Battery Power'"))
}

/// Return whether `pmset -g custom` reports Low Power Mode enabled.
#[cfg(target_os = "macos")]
fn pmset_low_power_mode() -> bool {
    command_stdout(&pmset_bin(), &["-g", "custom"]).is_some_and(|stdout| {
        stdout
            .lines()
            .any(|line| line.split_whitespace().collect::<Vec<_>>() == ["lowpowermode", "1"])
    })
}

/// Resolve the `pmset` binary, allowing tests to inject a fake implementation.
#[cfg(target_os = "macos")]
fn pmset_bin() -> String {
    std::env::var("MOADIM_PMSET_BIN").unwrap_or_else(|_| "pmset".to_string())
}

/// Run `program args...` and return stdout on success.
#[cfg(target_os = "macos")]
fn command_stdout(program: &str, args: &[&str]) -> Option<String> {
    let output = Command::new(program).args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8(output.stdout).ok()
}

#[cfg(test)]
#[path = "system_power_tests.rs"]
mod system_power_tests;
