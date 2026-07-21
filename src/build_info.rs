//! Compile-time build provenance: the crate version plus the git commit the
//! binary was built from and that commit's date.
//!
//! `--version` and `GET /health` surface these so an operator can tell exactly
//! which build is running — not just the crate version, which only changes on a
//! release tag and so can't distinguish two builds of the same `0.x.y` cut.
//!
//! `build.rs` resolves the git fields at compile time and substitutes
//! `"unknown"` when the source tree isn't a git checkout (e.g. a crates.io
//! tarball, where `.git` is absent), so a published build still compiles and
//! reports a sensible value.

/// Crate version from `Cargo.toml` (`CARGO_PKG_VERSION`).
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Short git commit SHA the binary was built from, with a `-dirty` suffix when
/// the working tree had uncommitted changes to tracked files at build time, or
/// `"unknown"` outside a git checkout.
pub const GIT_SHA: &str = env!("MOADIM_GIT_SHA");

/// Committer date (`YYYY-MM-DD`) of [`GIT_SHA`], or `"unknown"` outside a git checkout.
pub const BUILD_DATE: &str = env!("MOADIM_GIT_DATE");

/// Human-readable one-line version string, e.g. `0.1.0 (a1b2c3d 2026-06-19)`.
///
/// When the git provenance is unavailable (`GIT_SHA == "unknown"`), the suffix is
/// dropped and the bare crate version is returned.
pub fn long_version() -> String {
    format_version(VERSION, GIT_SHA, BUILD_DATE)
}

/// Pure formatting core for [`long_version`], split out so both the
/// git-available and the `"unknown"` fallback branches are unit-testable
/// regardless of how the test binary itself was built.
fn format_version(version: &str, sha: &str, date: &str) -> String {
    if sha == "unknown" {
        version.to_string()
    } else {
        format!("{version} ({sha} {date})")
    }
}

/// How often the running daemon checks whether the binary now sitting at its own `current_exe()`
/// path differs from the one this process was started from (#167). A daemon left running across an
/// in-place binary upgrade keeps executing the old compiled-in logic — including whatever it writes
/// into a routine's generated agent instructions with no signal to the operator that a restart
/// would pick up the newer build. Hourly is plenty: this is
/// an operator nudge, not a time-critical check, so it runs on its own cadence independent of
/// [`crate::routines::CLEANUP_INTERVAL`].
pub const VERSION_DRIFT_CHECK_INTERVAL: std::time::Duration =
    std::time::Duration::from_secs(60 * 60);

/// Compare `running`, this process's own version string, against what `exe` reports for
/// `--version` right now, returning a warning message when they differ.
///
/// `exe` is queried by spawning it rather than reading Cargo/git metadata off disk, so this
/// reflects whatever binary is *actually* installed at that path, however it was built. Returns
/// `None` when `exe` cannot be run, exits non-zero, prints nothing, or reports the same version —
/// i.e. whenever there is nothing an operator needs to act on.
pub(crate) fn version_drift_warning(exe: &std::path::Path, running: &str) -> Option<String> {
    let output = std::process::Command::new(exe)
        .arg("--version")
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let on_disk = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if on_disk.is_empty() || on_disk == running {
        return None;
    }
    Some(format!(
        "moadim: running build ({running}) differs from the build now on disk at {} ({on_disk}) — \
         restart the daemon (`moadim restart`) to pick up the update.",
        exe.display()
    ))
}

/// Log [`version_drift_warning`]'s message for `exe`/`running`, if any.
///
/// Split out from the periodic-task closure that calls it so that closure has a single
/// unconditional call site — the branching (and the `log::warn!` call site itself) lives here,
/// where `exe`/`running` are injectable and so directly unit-testable.
pub(crate) fn warn_on_drift(exe: &std::path::Path, running: &str) {
    if let Some(warning) = version_drift_warning(exe, running) {
        log::warn!("{warning}");
    }
}

#[cfg(test)]
#[path = "build_info_tests.rs"]
mod build_info_tests;
