#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use super::*;

#[test]
fn help_text_documents_every_accepted_flag() {
    let help = help_text();
    // Each alias `parse` accepts must be discoverable in `--help`, so the
    // documentation can't silently drift from the parser.
    for flag in [
        "-i",
        "--interactive",
        "-f",
        "--foreground", // foreground mode
        "-b",
        "--background",
        "-d",
        "--detach",
        "--daemon", // background mode
        "-h",
        "--help",
        "-V",
        "--version", // help & version
        "--json",
        "-q",
        "--quiet", // command flags
    ] {
        assert!(
            help.contains(flag),
            "help text is missing the `{flag}` flag that the parser accepts"
        );
    }
}
