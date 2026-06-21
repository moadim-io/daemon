//! Project task runner for the moadim workspace.
//!
//! Run tasks via the `cargo xtask <task>` alias (defined in `.cargo/config.toml`).
//! This crate is the home for repo tooling wrappers, so contributors don't need
//! to memorize each underlying tool's crate or binary name. New sibling tasks
//! (e.g. wrappers over `trunk` or `cargo-llvm-cov`) can be added here.

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, ExitStatus, Stdio};

fn main() -> ExitCode {
    let task = std::env::args().nth(1);
    match task.as_deref() {
        Some("spellcheck") => spellcheck(),
        Some(other) => {
            eprintln!("xtask: unknown task `{other}`");
            print_usage();
            ExitCode::from(2)
        }
        None => {
            eprintln!("xtask: no task given");
            print_usage();
            ExitCode::from(2)
        }
    }
}

/// Print the list of available tasks to stderr.
fn print_usage() {
    eprintln!("usage: cargo xtask <task>");
    eprintln!("tasks:");
    eprintln!("  spellcheck    install (if needed) and run `typos` over the repo");
}

/// Spell-check the whole tree with [`typos`](https://github.com/crate-ci/typos),
/// installing the `typos-cli` crate first if the `typos` binary is missing — so
/// a contributor never has to know that the crate is named `typos-cli` while the
/// binary is `typos`. Runs from the repo root so it reuses the same `typos.toml`
/// config as the pre-commit hook and the spellcheck CI workflow, keeping all
/// three in lockstep.
fn spellcheck() -> ExitCode {
    if !typos_installed() {
        eprintln!("xtask: `typos` not found; installing it with `cargo install typos-cli`...");
        let installed = Command::new("cargo")
            .args(["install", "typos-cli"])
            .status();
        match installed {
            Ok(status) if status.success() => {}
            Ok(status) => return exit_code_of(status),
            Err(err) => {
                eprintln!("xtask: failed to run `cargo install typos-cli`: {err}");
                return ExitCode::FAILURE;
            }
        }
    }

    match Command::new("typos").current_dir(repo_root()).status() {
        Ok(status) if status.success() => ExitCode::SUCCESS,
        Ok(status) => exit_code_of(status),
        Err(err) => {
            eprintln!("xtask: failed to run `typos`: {err}");
            ExitCode::FAILURE
        }
    }
}

/// Report whether the `typos` binary is callable on `PATH`.
fn typos_installed() -> bool {
    Command::new("typos")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

/// Resolve the repository root.
///
/// The `xtask` crate lives at `<repo>/xtask`, so the repo root is this crate's
/// manifest directory's parent.
fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
}

/// Translate a child process [`ExitStatus`] into an [`ExitCode`] for `xtask`,
/// falling back to a generic failure when the child was killed by a signal and
/// carries no numeric exit code.
fn exit_code_of(status: ExitStatus) -> ExitCode {
    match status.code() {
        Some(code) => ExitCode::from(code as u8),
        None => ExitCode::FAILURE,
    }
}
