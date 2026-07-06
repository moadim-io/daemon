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
    Ok(std::env::current_exe().expect("failed to resolve current executable path"))
}

/// Absolute path to the daemon log file the service manager redirects stdout/stderr to.
#[cfg(target_os = "macos")]
pub(super) fn daemon_log() -> PathBuf {
    crate::paths::daemon_log_file()
}
