#![allow(clippy::missing_docs_in_private_items)]

use super::*;

#[cfg(target_os = "macos")]
#[test]
fn plist_carries_label_program_args_and_supervision_keys() {
    let plist = render_plist(
        std::path::Path::new("/opt/moadim/bin/moadim"),
        std::path::Path::new("/Users/u/.config/moadim/daemon.log"),
    );
    assert!(plist.contains("<string>io.moadim.daemon</string>"));
    assert!(plist.contains("<string>/opt/moadim/bin/moadim</string>"));
    assert!(plist.contains("<string>--interactive</string>"));
    assert!(plist.contains("<key>RunAtLoad</key>"));
    assert!(plist.contains("<key>KeepAlive</key>"));
    assert!(plist.contains("/Users/u/.config/moadim/daemon.log"));
}

#[cfg(target_os = "macos")]
#[test]
fn plist_escapes_xml_metacharacters_in_paths() {
    let plist = render_plist(
        std::path::Path::new("/tmp/a&b<c>"),
        std::path::Path::new("/tmp/log"),
    );
    assert!(plist.contains("/tmp/a&amp;b&lt;c&gt;"));
    assert!(!plist.contains("a&b<c>"));
}

#[cfg(target_os = "macos")]
#[test]
fn xml_escape_covers_all_five_metacharacters() {
    assert_eq!(xml_escape("&<>\"'"), "&amp;&lt;&gt;&quot;&apos;");
}

#[cfg(target_os = "macos")]
#[test]
fn plist_path_is_under_launch_agents() {
    let path = plist_path().unwrap();
    assert!(path.ends_with("Library/LaunchAgents/io.moadim.daemon.plist"));
}

#[cfg(target_os = "linux")]
#[test]
fn unit_carries_exec_start_and_install_section() {
    let unit = render_unit(std::path::Path::new("/opt/moadim/bin/moadim"));
    assert!(unit.contains("ExecStart=/opt/moadim/bin/moadim --interactive"));
    assert!(unit.contains("[Install]"));
    assert!(unit.contains("WantedBy=default.target"));
    assert!(unit.contains("Restart=always"));
}

#[cfg(target_os = "linux")]
#[test]
fn unit_path_is_under_systemd_user() {
    let path = unit_path().unwrap();
    assert!(path.ends_with("systemd/user/moadim.service"));
}
