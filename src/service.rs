//! OS service installation: register the daemon to run at login/boot via the platform's service
//! manager, so scheduled routines keep firing across reboots.
//!
//! - **macOS** — a launchd LaunchAgent at `~/Library/LaunchAgents/io.moadim.daemon.plist`.
//! - **Linux** — a systemd *user* unit at `~/.config/systemd/user/moadim.service`.
//! - **Other platforms** (e.g. Windows) — not supported yet; `install`/`uninstall` report so.
//!
//! The generated service runs `moadim --interactive` (the foreground server) under the supervisor,
//! which handles backgrounding and restart-on-exit. Content generation and the registration argv are
//! pure functions so they can be unit-tested without touching the real service manager.

use std::path::{Path, PathBuf};

/// launchd label and systemd unit base name for the moadim daemon service.
pub const SERVICE_LABEL: &str = "io.moadim.daemon";

/// systemd user unit file name (the `[Install]` target registers under this name).
const SYSTEMD_UNIT_NAME: &str = "moadim.service";

/// The platform service manager moadim can register itself with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ServiceManager {
    /// macOS launchd (per-user LaunchAgent).
    Launchd,
    /// Linux systemd (per-user unit, `systemctl --user`).
    Systemd,
}

/// The service manager for the host platform, or `None` on an unsupported one (e.g. Windows).
fn current_manager() -> Option<ServiceManager> {
    if cfg!(target_os = "macos") {
        Some(ServiceManager::Launchd)
    } else if cfg!(target_os = "linux") {
        Some(ServiceManager::Systemd)
    } else {
        None
    }
}

/// Human-readable name of a service manager, for confirmation output.
fn manager_name(manager: ServiceManager) -> &'static str {
    match manager {
        ServiceManager::Launchd => "launchd (LaunchAgent)",
        ServiceManager::Systemd => "systemd (user)",
    }
}

/// The CLI program that drives a service manager.
fn manager_program(manager: ServiceManager) -> &'static str {
    match manager {
        ServiceManager::Launchd => "launchctl",
        ServiceManager::Systemd => "systemctl",
    }
}

/// Path to the service definition file for `manager`, or `None` if the home directory is unknown.
fn service_file_path(manager: ServiceManager) -> Option<PathBuf> {
    let home = dirs::home_dir()?;
    Some(match manager {
        ServiceManager::Launchd => home
            .join("Library/LaunchAgents")
            .join(format!("{SERVICE_LABEL}.plist")),
        ServiceManager::Systemd => home.join(".config/systemd/user").join(SYSTEMD_UNIT_NAME),
    })
}

/// Generate the launchd plist that runs `exe --interactive` at login, logging to `log_path`.
///
/// `RunAtLoad` starts it immediately and on every login; `KeepAlive` restarts it if it exits.
fn launchd_plist(exe: &str, log_path: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key>
  <string>{SERVICE_LABEL}</string>
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
  <string>{log_path}</string>
  <key>StandardErrorPath</key>
  <string>{log_path}</string>
</dict>
</plist>
"#
    )
}

/// Generate the systemd user unit that runs `exe --interactive`, restarting on failure and
/// appending output to `log_path`.
fn systemd_unit(exe: &str, log_path: &str) -> String {
    format!(
        "[Unit]\n\
         Description=moadim daemon (cron/MCP/REST server)\n\
         After=network.target\n\
         \n\
         [Service]\n\
         Type=simple\n\
         ExecStart={exe} --interactive\n\
         Restart=on-failure\n\
         StandardOutput=append:{log_path}\n\
         StandardError=append:{log_path}\n\
         \n\
         [Install]\n\
         WantedBy=default.target\n"
    )
}

/// The `manager_program(manager)` argv that registers (loads + enables) the service at `path`.
fn enable_argv(manager: ServiceManager, path: &Path) -> Vec<String> {
    match manager {
        ServiceManager::Launchd => vec![
            "load".to_string(),
            "-w".to_string(),
            path.display().to_string(),
        ],
        ServiceManager::Systemd => vec![
            "--user".to_string(),
            "enable".to_string(),
            "--now".to_string(),
            SYSTEMD_UNIT_NAME.to_string(),
        ],
    }
}

/// The `manager_program(manager)` argv that deregisters (disables + unloads) the service at `path`.
fn disable_argv(manager: ServiceManager, path: &Path) -> Vec<String> {
    match manager {
        ServiceManager::Launchd => vec![
            "unload".to_string(),
            "-w".to_string(),
            path.display().to_string(),
        ],
        ServiceManager::Systemd => vec![
            "--user".to_string(),
            "disable".to_string(),
            "--now".to_string(),
            SYSTEMD_UNIT_NAME.to_string(),
        ],
    }
}

/// Run a service-manager command, returning whether it exited successfully. A missing binary or a
/// non-zero exit yields `false` rather than aborting, so a best-effort step (e.g. unloading a
/// service that was never loaded) does not derail the surrounding operation.
fn run_service_command(program: &str, argv: &[String]) -> bool {
    std::process::Command::new(program)
        .args(argv)
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

/// Install moadim as an OS service for the current user: write the platform service file with the
/// resolved binary path, register it with the service manager, and print a confirmation.
pub fn install() -> anyhow::Result<()> {
    let Some(manager) = current_manager() else {
        anyhow::bail!(
            "`moadim install` is not supported on this platform yet (macOS launchd and Linux systemd only)"
        );
    };
    let exe = std::env::current_exe()?.to_string_lossy().into_owned();
    let log = crate::paths::daemon_log_file()
        .to_string_lossy()
        .into_owned();
    let path = service_file_path(manager)
        .ok_or_else(|| anyhow::anyhow!("could not resolve the home directory"))?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    if let Some(dir) = crate::paths::daemon_log_file().parent() {
        std::fs::create_dir_all(dir)?;
    }
    let contents = match manager {
        ServiceManager::Launchd => launchd_plist(&exe, &log),
        ServiceManager::Systemd => systemd_unit(&exe, &log),
    };
    std::fs::write(&path, contents)?;

    println!(
        "installed moadim as a {} service: {}",
        manager_name(manager),
        path.display()
    );
    let program = manager_program(manager);
    let argv = enable_argv(manager, &path);
    if run_service_command(program, &argv) {
        println!("  registered and started; it will run at login and restart if it exits");
    } else {
        println!(
            "  note: `{program} {}` did not complete — run it manually to start the service now",
            argv.join(" ")
        );
    }
    println!("  uninstall with: moadim uninstall");
    Ok(())
}

/// Uninstall the moadim OS service: deregister it (best-effort) and remove its service file.
pub fn uninstall() -> anyhow::Result<()> {
    let Some(manager) = current_manager() else {
        anyhow::bail!(
            "`moadim uninstall` is not supported on this platform yet (macOS launchd and Linux systemd only)"
        );
    };
    let path = service_file_path(manager)
        .ok_or_else(|| anyhow::anyhow!("could not resolve the home directory"))?;
    // Deregister first, ignoring failures (the service may not be loaded).
    let _ = run_service_command(manager_program(manager), &disable_argv(manager, &path));
    if path.exists() {
        std::fs::remove_file(&path)?;
        println!(
            "uninstalled moadim {} service: {}",
            manager_name(manager),
            path.display()
        );
    } else {
        println!("no moadim service file at {}", path.display());
    }
    Ok(())
}

#[cfg(test)]
#[path = "service_tests.rs"]
mod service_tests;
