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

/// Run an external command to completion, mapping a non-zero exit or spawn failure to an error.
#[cfg(any(target_os = "macos", target_os = "linux"))]
fn run(program: &str, args: &[&str]) -> anyhow::Result<()> {
    let status = std::process::Command::new(program)
        .args(args)
        .status()
        .map_err(|err| anyhow::anyhow!("failed to run `{program}`: {err}"))?;
    if !status.success() {
        anyhow::bail!("`{program}` exited with {status}");
    }
    Ok(())
}

// ── macOS (launchd) ─────────────────────────────────────────────────────────

/// launchd label, also the plist file stem (`io.moadim.daemon.plist`).
#[cfg(target_os = "macos")]
const LAUNCHD_LABEL: &str = "io.moadim.daemon";

/// Escape the five XML metacharacters so a filesystem path embeds safely in the plist `<string>`s.
#[cfg(target_os = "macos")]
fn xml_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

/// Absolute path to the per-user LaunchAgents plist for the moadim service.
#[cfg(target_os = "macos")]
fn plist_path() -> anyhow::Result<std::path::PathBuf> {
    let home = dirs::home_dir()
        .ok_or_else(|| anyhow::anyhow!("could not determine the home directory"))?;
    Ok(home
        .join("Library/LaunchAgents")
        .join(format!("{LAUNCHD_LABEL}.plist")))
}

/// Render the launchd property list for the moadim user agent.
///
/// `exe` is the absolute path to the `moadim` binary; `log` is where launchd writes its stdout and
/// stderr. The agent runs `moadim --interactive` so launchd supervises it directly.
#[cfg(target_os = "macos")]
fn render_plist(exe: &std::path::Path, log: &std::path::Path) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key>
  <string>{label}</string>
  <key>ProgramArguments</key>
  <array>
    <string>{exe}</string>
    <string>--interactive</string>
  </array>
  <key>RunAtLoad</key>
  <true/>
  <key>KeepAlive</key>
  <true/>
  <key>StandardOutPath</key>
  <string>{log}</string>
  <key>StandardErrorPath</key>
  <string>{log}</string>
</dict>
</plist>
"#,
        label = LAUNCHD_LABEL,
        exe = xml_escape(&exe.display().to_string()),
        log = xml_escape(&log.display().to_string()),
    )
}

/// Write the LaunchAgent plist for the running binary and load it with launchd.
#[cfg(target_os = "macos")]
pub fn install() -> anyhow::Result<()> {
    let exe = std::env::current_exe()?;
    let log = crate::paths::daemon_log_file();
    let plist = plist_path()?;
    if let Some(dir) = plist.parent() {
        std::fs::create_dir_all(dir)?;
    }
    if let Some(dir) = log.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(&plist, render_plist(&exe, &log))?;

    let plist_arg = plist.display().to_string();
    // Unload any earlier copy first (ignored when not loaded), then load with -w to enable it.
    let _ = run("launchctl", &["unload", &plist_arg]);
    run("launchctl", &["load", "-w", &plist_arg])?;

    println!("moadim installed as a launchd agent ({LAUNCHD_LABEL})");
    println!("  plist   {}", plist.display());
    println!("  logs    {}", log.display());
    println!("  status  launchctl list | grep {LAUNCHD_LABEL}");
    Ok(())
}

/// Unload the LaunchAgent (if loaded) and delete its plist.
#[cfg(target_os = "macos")]
pub fn uninstall() -> anyhow::Result<()> {
    let plist = plist_path()?;
    if plist.exists() {
        let plist_arg = plist.display().to_string();
        let _ = run("launchctl", &["unload", "-w", &plist_arg]);
        std::fs::remove_file(&plist)?;
        println!("moadim launchd agent removed ({})", plist.display());
    } else {
        println!(
            "moadim launchd agent is not installed (no plist at {})",
            plist.display()
        );
    }
    Ok(())
}

// ── Linux (systemd user) ─────────────────────────────────────────────────────

/// systemd user unit file name for the moadim service.
#[cfg(target_os = "linux")]
const SYSTEMD_UNIT_NAME: &str = "moadim.service";

/// Absolute path to the systemd *user* unit file for the moadim service.
#[cfg(target_os = "linux")]
fn unit_path() -> anyhow::Result<std::path::PathBuf> {
    let base = dirs::config_dir()
        .ok_or_else(|| anyhow::anyhow!("could not determine the config directory"))?;
    Ok(base.join("systemd/user").join(SYSTEMD_UNIT_NAME))
}

/// Render the systemd user unit for the moadim service.
///
/// `exe` is the absolute path to the `moadim` binary. The service runs `moadim --interactive` in the
/// foreground (`Type=simple`) so systemd supervises it and restarts it on failure.
#[cfg(target_os = "linux")]
fn render_unit(exe: &std::path::Path) -> String {
    format!(
        "[Unit]\n\
         Description=moadim cron/MCP/REST daemon\n\
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

/// Write the systemd user unit for the running binary and enable + start it.
#[cfg(target_os = "linux")]
pub fn install() -> anyhow::Result<()> {
    let exe = std::env::current_exe()?;
    let unit = unit_path()?;
    if let Some(dir) = unit.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(&unit, render_unit(&exe))?;

    run("systemctl", &["--user", "daemon-reload"])?;
    run(
        "systemctl",
        &["--user", "enable", "--now", SYSTEMD_UNIT_NAME],
    )?;

    println!("moadim installed as a systemd user service ({SYSTEMD_UNIT_NAME})");
    println!("  unit    {}", unit.display());
    println!("  status  systemctl --user status {SYSTEMD_UNIT_NAME}");
    Ok(())
}

/// Disable + stop the systemd user service (if present) and delete its unit file.
#[cfg(target_os = "linux")]
pub fn uninstall() -> anyhow::Result<()> {
    let unit = unit_path()?;
    if unit.exists() {
        let _ = run(
            "systemctl",
            &["--user", "disable", "--now", SYSTEMD_UNIT_NAME],
        );
        std::fs::remove_file(&unit)?;
        let _ = run("systemctl", &["--user", "daemon-reload"]);
        println!("moadim systemd user service removed ({})", unit.display());
    } else {
        println!(
            "moadim systemd user service is not installed (no unit at {})",
            unit.display()
        );
    }
    Ok(())
}

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
#[path = "service_tests.rs"]
mod service_tests;
