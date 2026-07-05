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
                interactive: true
            },
            "flag {flag} should select interactive restart"
        );
    }
}
