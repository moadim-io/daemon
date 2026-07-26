#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use super::*;

#[test]
fn restart_command_interactive_flag() {
    for flag in ["-i", "--interactive"] {
        assert_eq!(
            parse(argv(&["restart", flag])),
            Command::Restart {
                json: false,
                quiet: false,
                interactive: true
            },
            "flag {flag} should select interactive restart"
        );
    }
}

#[test]
fn restart_json_and_quiet_flags_parse() {
    assert_eq!(
        parse(argv(&["restart", "--json"])),
        Command::Restart {
            json: true,
            quiet: false,
            interactive: false
        }
    );
    for flag in ["--quiet", "-q"] {
        assert_eq!(
            parse(argv(&["restart", flag])),
            Command::Restart {
                json: false,
                quiet: true,
                interactive: false
            },
            "flag {flag}"
        );
    }
    assert_eq!(
        parse(argv(&["restart", "--json", "-q"])),
        Command::Restart {
            json: true,
            quiet: true,
            interactive: false
        }
    );
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
#[test]
fn should_hint_install_only_when_unmarked_and_not_installed() {
    assert!(
        should_hint_install(false, false),
        "no marker + not installed -> hint"
    );
    assert!(
        !should_hint_install(true, false),
        "already shown (marker present) -> don't repeat"
    );
    assert!(
        !should_hint_install(false, true),
        "already installed -> nothing to hint"
    );
    assert!(!should_hint_install(true, true));
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
#[test]
fn maybe_hint_install_writes_marker_when_not_installed() {
    let home = crate::cli::cli_spawn_tests::temp_home("install-hint-fresh");
    let _home =
        crate::cli::cli_spawn_tests::EnvGuard::set("MOADIM_HOME_OVERRIDE", home.to_str().unwrap());
    let marker = crate::paths::install_prompt_marker_path();
    assert!(!marker.exists());

    maybe_hint_install();
    assert!(
        marker.exists(),
        "a fresh, uninstalled start must record that the hint fired"
    );

    // A second start must not error or duplicate work now that the marker is present.
    maybe_hint_install();
    assert!(marker.exists());
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn restart_json_reports_old_new_pid_and_address() {
    let rotated: serde_json::Value = serde_json::from_str(&restart_json(Some(123), 456)).unwrap();
    assert_eq!(rotated["old"], 123);
    assert_eq!(rotated["new"], 456);
    assert_eq!(rotated["address"], bind_addr());

    // `old` is null when nothing was running, mirroring the `none` rotation rendering.
    let fresh: serde_json::Value = serde_json::from_str(&restart_json(None, 456)).unwrap();
    assert!(fresh["old"].is_null());
    assert_eq!(fresh["new"], 456);
}
