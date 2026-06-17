//! Tests for OS-service file generation and registration argv (the pure, platform-independent parts).

#![allow(clippy::missing_docs_in_private_items)]

use super::*;
use std::path::Path;

#[test]
fn launchd_plist_embeds_label_exe_and_log() {
    let plist = launchd_plist("/usr/local/bin/moadim", "/home/u/.config/moadim/daemon.log");
    assert!(plist.contains(&format!("<string>{SERVICE_LABEL}</string>")));
    assert!(plist.contains("<string>/usr/local/bin/moadim</string>"));
    // Runs the foreground server so launchd supervises it.
    assert!(plist.contains("<string>--interactive</string>"));
    // Starts at login and restarts on exit.
    assert!(plist.contains("<key>RunAtLoad</key>"));
    assert!(plist.contains("<key>KeepAlive</key>"));
    // Logs to the daemon log on both streams.
    assert!(plist.contains("<key>StandardOutPath</key>"));
    assert!(plist.contains("<key>StandardErrorPath</key>"));
    assert!(plist.contains("<string>/home/u/.config/moadim/daemon.log</string>"));
    // Well-formed plist header.
    assert!(plist.trim_start().starts_with("<?xml"));
}

#[test]
fn systemd_unit_embeds_exec_start_restart_and_target() {
    let unit = systemd_unit("/usr/local/bin/moadim", "/home/u/.config/moadim/daemon.log");
    assert!(unit.contains("ExecStart=/usr/local/bin/moadim --interactive"));
    assert!(unit.contains("Restart=on-failure"));
    // User-session install target.
    assert!(unit.contains("WantedBy=default.target"));
    assert!(unit.contains("StandardOutput=append:/home/u/.config/moadim/daemon.log"));
    assert!(unit.contains("[Unit]") && unit.contains("[Service]") && unit.contains("[Install]"));
}

#[test]
fn manager_program_and_name_per_manager() {
    assert_eq!(manager_program(ServiceManager::Launchd), "launchctl");
    assert_eq!(manager_program(ServiceManager::Systemd), "systemctl");
    assert!(manager_name(ServiceManager::Launchd).contains("launchd"));
    assert!(manager_name(ServiceManager::Systemd).contains("systemd"));
}

#[test]
fn enable_argv_loads_launchd_plist_path() {
    let argv = enable_argv(
        ServiceManager::Launchd,
        Path::new("/p/io.moadim.daemon.plist"),
    );
    assert_eq!(argv, vec!["load", "-w", "/p/io.moadim.daemon.plist"]);
}

#[test]
fn enable_argv_enables_systemd_unit_by_name() {
    let argv = enable_argv(ServiceManager::Systemd, Path::new("/ignored"));
    assert_eq!(argv, vec!["--user", "enable", "--now", SYSTEMD_UNIT_NAME]);
}

#[test]
fn disable_argv_unloads_launchd_plist_path() {
    let argv = disable_argv(
        ServiceManager::Launchd,
        Path::new("/p/io.moadim.daemon.plist"),
    );
    assert_eq!(argv, vec!["unload", "-w", "/p/io.moadim.daemon.plist"]);
}

#[test]
fn disable_argv_disables_systemd_unit_by_name() {
    let argv = disable_argv(ServiceManager::Systemd, Path::new("/ignored"));
    assert_eq!(argv, vec!["--user", "disable", "--now", SYSTEMD_UNIT_NAME]);
}

#[test]
fn service_file_path_points_at_platform_location() {
    // launchd plist lives under ~/Library/LaunchAgents with the label filename.
    if let Some(plist) = service_file_path(ServiceManager::Launchd) {
        let plist = plist.display().to_string();
        assert!(plist.contains("Library/LaunchAgents"));
        assert!(plist.ends_with(&format!("{SERVICE_LABEL}.plist")));
    }
    // systemd unit lives under ~/.config/systemd/user with the unit name.
    if let Some(unit) = service_file_path(ServiceManager::Systemd) {
        let unit = unit.display().to_string();
        assert!(unit.contains(".config/systemd/user"));
        assert!(unit.ends_with(SYSTEMD_UNIT_NAME));
    }
}

#[test]
fn run_service_command_false_for_missing_binary() {
    // A nonexistent program cannot succeed; the helper reports false rather than panicking.
    assert!(!run_service_command(
        "moadim-no-such-binary-xyzzy",
        &["--version".to_string()]
    ));
}

#[test]
fn current_manager_is_supported_on_unix_test_host() {
    // The CI/dev hosts are macOS or Linux, both supported.
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    assert!(current_manager().is_some());
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    assert!(current_manager().is_none());
}
