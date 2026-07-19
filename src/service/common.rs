//! Shared helpers for the platform-specific service installers.

use std::path::PathBuf;

/// Run an external command to completion, mapping a non-zero exit or spawn failure to an error.
pub(super) fn run(program: &str, args: &[&str]) -> anyhow::Result<()> {
    let status = std::process::Command::new(program)
        .args(args)
        .status()
        .map_err(|err| anyhow::anyhow!("failed to run `{program}`: {err}"))?;
    if !status.success() {
        anyhow::bail!("`{program}` exited with {status}");
    }
    Ok(())
}

/// Absolute path to the currently running `moadim` binary, supervised by the service manager.
pub(super) fn moadim_exe() -> anyhow::Result<PathBuf> {
    current_exe_path(std::env::current_exe)
}

/// Resolve `current_exe` through a small seam so the error mapping stays testable.
fn current_exe_path(
    current_exe: impl FnOnce() -> Result<PathBuf, std::io::Error>,
) -> anyhow::Result<PathBuf> {
    current_exe().map_err(|err| current_exe_error(&err))
}

/// Format the `current_exe` failure into the error message the callers already log.
fn current_exe_error(err: &std::io::Error) -> anyhow::Error {
    let message = err.to_string();
    anyhow::anyhow!("failed to resolve current executable path: {message}")
}

#[cfg(test)]
#[path = "common_tests.rs"]
mod common_tests;

/// Absolute path to the daemon log file the service manager redirects stdout/stderr to.
#[cfg(target_os = "macos")]
pub(super) fn daemon_log() -> PathBuf {
    crate::paths::daemon_log_file()
}
