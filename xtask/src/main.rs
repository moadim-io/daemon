//! Workspace task runner — the home for project tooling wrappers.
//!
//! Run tasks through the cargo alias defined in `.cargo/config.toml`:
//!
//! ```sh
//! cargo xtask spellcheck
//! ```
//!
//! Today the only task is `spellcheck`, which installs (if needed) and runs the
//! [`typos`](https://github.com/crate-ci/typos) spell-checker over the repo so a contributor never
//! has to remember that the crate is `typos-cli` while the binary is `typos`. New project task
//! wrappers (the CONTRIBUTING table lists more `cargo install` tools) belong here too.

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};

/// The crate that provides the spell-checker — note it differs from the binary name below, which is
/// exactly the "what's it called again?" friction this wrapper removes.
const TYPOS_CRATE: &str = "typos-cli";
/// The binary `TYPOS_CRATE` installs, and the spell-checker this task invokes.
const TYPOS_BIN: &str = "typos";

fn main() -> ExitCode {
    match std::env::args().nth(1).as_deref() {
        Some("spellcheck") => spellcheck(),
        Some(task) => {
            eprintln!("xtask: unknown task '{task}'");
            usage();
            ExitCode::from(2)
        }
        None => {
            eprintln!("xtask: no task given");
            usage();
            ExitCode::from(2)
        }
    }
}

/// Print the available tasks to stderr.
fn usage() {
    eprintln!(
        "usage: cargo xtask <task>\n\
         \n\
         tasks:\n\
         \x20   spellcheck    install (if needed) and run `typos` over the repo"
    );
}

/// Spell-check the tree with `typos`, installing `typos-cli` first when the binary is absent so the
/// contributor never has to know the crate/binary name. Runs from the workspace root so `typos`
/// picks up the repo's `typos.toml` — the same config the pre-commit hook and the spellcheck CI
/// gate use, so the three can't drift apart.
fn spellcheck() -> ExitCode {
    if !typos_installed() {
        eprintln!("`{TYPOS_BIN}` not found — installing `{TYPOS_CRATE}`…");
        if !run(Command::new("cargo").args(["install", TYPOS_CRATE])) {
            eprintln!("xtask: failed to install `{TYPOS_CRATE}`");
            return ExitCode::FAILURE;
        }
    }
    if run(Command::new(TYPOS_BIN).current_dir(workspace_root())) {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

/// Whether the `typos` binary is on the `PATH`, probed quietly with `typos --version`.
fn typos_installed() -> bool {
    Command::new(TYPOS_BIN)
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

/// Run `command` with inherited stdio, returning whether it exited successfully (and `false` if it
/// could not be spawned at all).
fn run(command: &mut Command) -> bool {
    command
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

/// The workspace root: the parent of this `xtask` crate's directory, resolved at compile time so the
/// task works regardless of the shell's current directory.
fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
}
