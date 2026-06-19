//! Build script: generates `schemas/job.schema.json` and the embedded UI HTML.

#[path = "src/build/mod.rs"]
mod build;

fn main() {
    println!("cargo:rerun-if-changed=ui/index.html");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=schemas/job.schema.json");
    // Re-stamp the embedded git provenance whenever HEAD moves (new commit or
    // checkout). Harmless when `.git/HEAD` is absent (e.g. a crates.io tarball):
    // cargo just watches a path that never changes.
    println!("cargo:rerun-if-changed=.git/HEAD");

    // Embed the commit the binary is built from so `--version` and `GET /health`
    // can identify the exact build, not just the crate version. Falls back to
    // "unknown" outside a git checkout so published builds still compile.
    println!(
        "cargo:rustc-env=MOADIM_GIT_SHA={}",
        git_output(&["rev-parse", "--short", "HEAD"])
    );
    println!(
        "cargo:rustc-env=MOADIM_GIT_DATE={}",
        git_output(&["show", "-s", "--format=%cs", "HEAD"])
    );

    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    build::run(&manifest_dir);
}

/// Run `git <args>` and return its trimmed stdout, or `"unknown"` when git is
/// missing, exits non-zero, or the output is empty (e.g. building from a
/// crates.io tarball with no `.git` directory).
fn git_output(args: &[&str]) -> String {
    std::process::Command::new("git")
        .args(args)
        .output()
        .ok()
        .filter(|out| out.status.success())
        .and_then(|out| String::from_utf8(out.stdout).ok())
        .map(|stdout| stdout.trim().to_string())
        .filter(|stdout| !stdout.is_empty())
        .unwrap_or_else(|| "unknown".to_string())
}
