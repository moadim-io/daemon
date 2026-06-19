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

/// Short git commit SHA the binary was built from, or `"unknown"` outside a git checkout.
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

#[cfg(test)]
#[path = "build_info_tests.rs"]
mod build_info_tests;
