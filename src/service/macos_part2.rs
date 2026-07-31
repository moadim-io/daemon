
/// Write the `LaunchAgent` plist for the running binary and load it with launchd.
pub fn install() -> anyhow::Result<()> {
    let exe = moadim_exe()?;
    let log = daemon_log();
    // Propagate an undeterminable home directory with `?` instead of `.expect()`-ing it into a
    // panic. Mirrors the Linux backend's `install()`, which propagates `unit_path()` the same way.
    let home = crate::paths::home();
    let plist = plist_path_from_home(home.clone())?;
    // The `?` above already returned if `home` were `None` (see `plist_path_from_home`), so this
    // can never panic.
    #[allow(
        clippy::expect_used,
        reason = "the `?` above already returned if `home` were `None` (see \
                  `plist_path_from_home`), so this is a proven invariant, not a real failure path"
    )]
    let home = home.expect("plist_path_from_home errors before this point when home is None");
    let working_dir = std::env::current_dir()?;
    write_plist(&plist, &exe, &log, &home, &working_dir)?;
    reload_agent(&plist)?;
    report_installed(&plist, &log);
    request_automation_permission();
    Ok(())
}

/// Trigger the macOS TCC "administer your computer" prompt now, while the user is present at the
/// terminal, so the background daemon never has to ask for it mid-run.
///
/// Sends a harmless Apple Event to System Events (list running process names). If permission is
/// already granted this is a no-op; if not, the dialog appears once here and is remembered forever.
fn request_automation_permission() {
    println!(
        "  hint    if macOS asks \"moadim would like to administer your computer\", click OK — \
granting it here prevents background interruptions"
    );
    let _ = std::process::Command::new("osascript")
        .args([
            "-e",
            "tell application \"System Events\" to get name of every process",
        ])
        .output();
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
