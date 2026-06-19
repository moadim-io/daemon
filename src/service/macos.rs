//! macOS launchd `LaunchAgent`: render the plist, write it, and (un)load it with `launchctl`.

use std::path::{Path, PathBuf};

use super::common::{daemon_log, moadim_exe, run};

/// launchd label, also the plist file stem (`io.moadim.daemon.plist`).
const LAUNCHD_LABEL: &str = "io.moadim.daemon";

/// The `launchctl` executable, overridable via `MOADIM_LAUNCHCTL_BIN` so tests can substitute a
/// no-op shim instead of mutating the real launchd session. Mirrors the `MOADIM_CRONTAB_BIN` seam.
pub(super) fn launchctl_bin() -> String {
    std::env::var("MOADIM_LAUNCHCTL_BIN").unwrap_or_else(|_| "launchctl".to_string())
}

/// Escape the five XML metacharacters so a filesystem path embeds safely in the plist `<string>`s.
pub(super) fn xml_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

/// Absolute path to the per-user `LaunchAgents` plist for the moadim service.
///
/// Resolves home through [`crate::paths::home`] so the `MOADIM_HOME_OVERRIDE` test seam redirects
/// the plist under a tempdir instead of the developer's real `~/Library/LaunchAgents`.
pub(super) fn plist_path() -> anyhow::Result<PathBuf> {
    plist_path_from_home(crate::paths::home())
}

/// Resolve the `LaunchAgents` plist path under `home`, erroring when the home directory is unknown.
pub(super) fn plist_path_from_home(home: Option<PathBuf>) -> anyhow::Result<PathBuf> {
    let home = home.ok_or_else(|| anyhow::anyhow!("could not determine the home directory"))?;
    Ok(home
        .join("Library/LaunchAgents")
        .join(format!("{LAUNCHD_LABEL}.plist")))
}

/// Render the launchd property list for the moadim user agent.
///
/// `exe` is the absolute path to the `moadim` binary; `log` is where launchd writes its stdout and
/// stderr. The agent runs `moadim --interactive` so launchd supervises it directly.
pub(super) fn render_plist(exe: &Path, log: &Path) -> String {
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

/// Render the plist for `exe`/`log` and write it (creating parent dirs) to `plist`.
pub(super) fn write_plist(plist: &Path, exe: &Path, log: &Path) -> anyhow::Result<()> {
    if let Some(dir) = plist.parent() {
        std::fs::create_dir_all(dir)?;
    }
    if let Some(dir) = log.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(plist, render_plist(exe, log))?;
    Ok(())
}

/// Reload the agent with launchd: unload any earlier copy (ignored when not loaded), then load
/// with `-w` to enable it.
fn reload_agent(plist: &Path) -> anyhow::Result<()> {
    let plist_arg = plist.display().to_string();
    let launchctl = launchctl_bin();
    let _ = run(&launchctl, &["unload", &plist_arg]);
    run(&launchctl, &["load", "-w", &plist_arg])
}

/// Print the post-install summary (paths and a status hint).
fn report_installed(plist: &Path, log: &Path) {
    println!("moadim installed as a launchd agent ({LAUNCHD_LABEL})");
    println!("  plist   {}", plist.display());
    println!("  logs    {}", log.display());
    println!("  status  launchctl list | grep {LAUNCHD_LABEL}");
}

/// Write the `LaunchAgent` plist for the running binary and load it with launchd.
pub fn install() -> anyhow::Result<()> {
    let exe = moadim_exe()?;
    let log = daemon_log();
    let plist = plist_path()?;
    write_plist(&plist, &exe, &log)?;
    reload_agent(&plist)?;
    report_installed(&plist, &log);
    Ok(())
}

/// Unload the `LaunchAgent` (if loaded) and delete its plist.
pub fn uninstall() -> anyhow::Result<()> {
    let plist = plist_path()?;
    if plist.exists() {
        let plist_arg = plist.display().to_string();
        let _ = run(&launchctl_bin(), &["unload", "-w", &plist_arg]);
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
