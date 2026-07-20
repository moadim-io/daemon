//! Build script: generates the embedded UI HTML.
#![allow(
    clippy::expect_used,
    reason = "this crate root is the build script, not the daemon binary — a `.expect()` panic \
              here just aborts `cargo build` with a message, the normal and desired failure mode \
              for a build script, not the graceful-shutdown risk `expect_used` guards against in \
              the long-running server"
)]

#[path = "src/build/mod.rs"]
mod build;

fn main() {
    println!("cargo:rerun-if-changed=client/index.html");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=schemas/routine.schema.json");
    // Re-stamp the embedded git provenance whenever HEAD moves (new commit or
    // checkout) or the index changes (staging). Harmless when these paths are
    // absent (e.g. a crates.io tarball): cargo just watches paths that never
    // change. A purely unstaged edit to a tracked file moves neither
    // `.git/HEAD` nor `.git/index`, so its `-dirty` marker is re-stamped on the
    // next build that does touch HEAD/index (or after a `cargo clean`); staged
    // or committed changes always re-stamp immediately.
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=.git/index");

    // Embed the commit the binary is built from so `--version` and `GET /health`
    // can identify the exact build, not just the crate version. Falls back to
    // "unknown" outside a git checkout so published builds still compile. A
    // working tree with uncommitted changes to tracked files gets a `-dirty`
    // suffix (e.g. `a1b2c3d-dirty`), the conventional `git describe --dirty`
    // signal, so a build from a mutated tree never reports a clean SHA.
    let mut git_sha = git_output(&["rev-parse", "--short", "HEAD"]);
    if git_sha != "unknown" && git_is_dirty() {
        git_sha.push_str("-dirty");
    }
    println!("cargo:rustc-env=MOADIM_GIT_SHA={git_sha}");
    println!(
        "cargo:rustc-env=MOADIM_GIT_DATE={}",
        git_output(&["show", "-s", "--format=%cs", "HEAD"])
    );

    let manifest_dir =
        std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is always set by cargo");
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

/// Return `true` when the working tree has uncommitted changes to *tracked*
/// files (staged or unstaged). Untracked files are ignored — they are not part
/// of the built source. `git diff --quiet HEAD` exits 1 when tracked files
/// differ from HEAD and 0 when the tree is clean; any other outcome (git
/// missing, no commits, errors) is treated as not-dirty so a non-git build
/// never spuriously reports `-dirty`.
fn git_is_dirty() -> bool {
    std::process::Command::new("git")
        .args(["diff", "--quiet", "HEAD", "--"])
        .output()
        .is_ok_and(|out| out.status.code() == Some(1))
}
