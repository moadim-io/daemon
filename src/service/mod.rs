//! `moadim install` / `moadim uninstall`: register the daemon as an OS service so it starts at login
//! and is restarted on crash, keeping scheduled routines firing across reboots.
//!
//! - **macOS** — a per-user launchd LaunchAgent at `~/Library/LaunchAgents/io.moadim.daemon.plist`.
//! - **Linux** — a systemd *user* service at `~/.config/systemd/user/moadim.service`.
//! - **Other platforms** — `install`/`uninstall` return an unsupported-platform error.
//!
//! The service runs the daemon in the foreground (`moadim --interactive`) so the service manager —
//! not moadim's own background-detach logic — owns supervision (RunAtLoad/KeepAlive on launchd,
//! `Restart=always` on systemd).

#[cfg(any(target_os = "macos", target_os = "linux"))]
mod common;

#[cfg(target_os = "macos")]
mod macos;

#[cfg(target_os = "linux")]
mod linux;

#[cfg(target_os = "macos")]
pub use macos::{install, uninstall};

#[cfg(target_os = "linux")]
pub use linux::{install, uninstall};

// Bring the platform render/path helpers into this module's namespace so the shared
// `service_tests` submodule can reach them via `super::*` regardless of which OS compiles.
#[cfg(all(test, target_os = "linux"))]
use linux::{
    linger_marker_path, loginctl_bin, render_unit, systemctl_bin, unit_path,
    unit_path_from_config_dir, write_unit,
};
#[cfg(all(test, target_os = "macos"))]
use macos::{
    launchctl_bin, plist_path, plist_path_from_home, render_plist, write_plist, xml_escape,
};

// ── Unsupported platforms ────────────────────────────────────────────────────

/// Service installation is not implemented for this platform.
#[cfg(not(any(target_os = "macos", target_os = "linux")))]
pub fn install() -> anyhow::Result<()> {
    anyhow::bail!("`moadim install` is not supported on this platform yet")
}

/// Service uninstallation is not implemented for this platform.
#[cfg(not(any(target_os = "macos", target_os = "linux")))]
pub fn uninstall() -> anyhow::Result<()> {
    anyhow::bail!("`moadim uninstall` is not supported on this platform yet")
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod service_tests;
