//! Linux systemd *user* service: render the unit, write it, and enable/disable it with `systemctl`.

use std::path::{Path, PathBuf};

use super::common::{moadim_exe, run};

/// systemd user unit file name for the moadim service.
const SYSTEMD_UNIT_NAME: &str = "moadim.service";

/// The `systemctl` executable, overridable via `MOADIM_SYSTEMCTL_BIN` so tests can substitute a
/// no-op shim instead of mutating the developer's live systemd user session. Mirrors the
/// `MOADIM_LAUNCHCTL_BIN` seam on macOS.
pub(super) fn systemctl_bin() -> String {
    if let Ok(bin) = std::env::var("MOADIM_SYSTEMCTL_BIN") {
        return bin;
    }
    // In test builds, never fall back to the real `systemctl`: a test that forgets to wire up the
    // `MOADIM_SYSTEMCTL_BIN` shim must not mutate the developer's live systemd session. The guard
    // path does not exist, so the eventual spawn fails harmlessly. Mirrors the `launchctl_bin()`
    // guard on macOS.
    #[cfg(test)]
    let fallback = "/nonexistent/moadim-test-systemctl-guard".to_string();
    #[cfg(not(test))]
    let fallback = "systemctl".to_string();
    fallback
}

/// Absolute path to the systemd *user* unit file for the moadim service.
pub(super) fn unit_path() -> anyhow::Result<PathBuf> {
    unit_path_from_config_dir(dirs::config_dir())
}

/// Resolve the systemd user unit path under `config_dir`, erroring when it's unknown.
pub(super) fn unit_path_from_config_dir(config_dir: Option<PathBuf>) -> anyhow::Result<PathBuf> {
    let base =
        config_dir.ok_or_else(|| anyhow::anyhow!("could not determine the config directory"))?;
    Ok(base.join("systemd/user").join(SYSTEMD_UNIT_NAME))
}

/// Render the systemd user unit for the moadim service.
///
/// `exe` is the absolute path to the `moadim` binary. The service runs `moadim --interactive` in the
/// foreground (`Type=simple`) so systemd supervises it and restarts it on failure.
pub(super) fn render_unit(exe: &Path) -> String {
    format!(
        "[Unit]\n\
         Description=moadim routine scheduler / MCP/REST daemon\n\
         After=network.target\n\
         \n\
         [Service]\n\
         Type=simple\n\
         ExecStart={exe} --interactive\n\
         Restart=always\n\
         RestartSec=5\n\
         \n\
         [Install]\n\
         WantedBy=default.target\n",
        exe = exe.display(),
    )
}

/// Render the unit for `exe` and write it (creating parent dirs) to `unit`.
pub(super) fn write_unit(unit: &Path, exe: &Path) -> anyhow::Result<()> {
    if let Some(dir) = unit.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(unit, render_unit(exe))?;
    Ok(())
}

/// Reload systemd's user manager, then enable and start the unit immediately.
fn enable_unit() -> anyhow::Result<()> {
    let systemctl = systemctl_bin();
    run(&systemctl, &["--user", "daemon-reload"])?;
    run(
        &systemctl,
        &["--user", "enable", "--now", SYSTEMD_UNIT_NAME],
    )
}

/// Print the post-install summary (path and a status hint).
fn report_installed(unit: &Path) {
    println!("moadim installed as a systemd user service ({SYSTEMD_UNIT_NAME})");
    println!("  unit    {}", unit.display());
    println!("  status  systemctl --user status {SYSTEMD_UNIT_NAME}");
}

/// Write the systemd user unit for the running binary and enable + start it.
pub fn install() -> anyhow::Result<()> {
    let exe = moadim_exe()?;
    let unit = unit_path()?;
    write_unit(&unit, &exe)?;
    enable_unit()?;
    report_installed(&unit);
    Ok(())
}

/// Disable + stop the systemd user service (if present) and delete its unit file.
pub fn uninstall() -> anyhow::Result<()> {
    let unit = unit_path()?;
    if unit.exists() {
        let systemctl = systemctl_bin();
        let _ = run(
            &systemctl,
            &["--user", "disable", "--now", SYSTEMD_UNIT_NAME],
        );
        std::fs::remove_file(&unit)?;
        let _ = run(&systemctl, &["--user", "daemon-reload"]);
        println!("moadim systemd user service removed ({})", unit.display());
    } else {
        println!(
            "moadim systemd user service is not installed (no unit at {})",
            unit.display()
        );
    }
    Ok(())
}
