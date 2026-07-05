//! Tests for [`super`] build-provenance formatting.

use super::{
    format_version, long_version, version_drift_warning, warn_on_drift, BUILD_DATE, GIT_SHA,
    VERSION,
};

/// Write an executable shell script named `name` under a fresh temp dir, containing `body`, and
/// return its path. Used to stand in for the on-disk `moadim` binary [`version_drift_warning`]
/// shells out to, without depending on this test binary's own `--version` behavior.
fn write_script(name: &str, body: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("moadim-build-info-test-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(name);
    std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    path
}

#[test]
fn format_version_includes_sha_and_date_when_known() {
    assert_eq!(
        format_version("0.1.0", "a1b2c3d", "2026-06-19"),
        "0.1.0 (a1b2c3d 2026-06-19)"
    );
}

#[test]
fn format_version_drops_suffix_when_sha_unknown() {
    // Outside a git checkout build.rs sets the SHA to "unknown"; the provenance
    // suffix must be dropped, leaving only the bare crate version.
    assert_eq!(format_version("0.1.0", "unknown", "unknown"), "0.1.0");
}

#[test]
fn format_version_carries_dirty_suffix_through() {
    // A build from a tree with uncommitted tracked changes stamps "<sha>-dirty"
    // (build.rs). Formatting passes it through verbatim, so --version / health /
    // MCP provenance surface the dirty marker instead of a misleading clean SHA.
    assert_eq!(
        format_version("0.1.0", "a1b2c3d-dirty", "2026-06-19"),
        "0.1.0 (a1b2c3d-dirty 2026-06-19)"
    );
}

#[test]
fn long_version_starts_with_the_crate_version() {
    // Regardless of whether this test binary was built inside a git checkout,
    // the rendered string always begins with the crate version.
    assert!(long_version().starts_with(VERSION));
}

#[test]
fn long_version_reflects_the_embedded_git_fields() {
    // Mirror the production branch: when the SHA is embedded, the rendered line
    // carries both git fields; otherwise it is exactly the crate version.
    if GIT_SHA == "unknown" {
        assert_eq!(long_version(), VERSION);
    } else {
        assert_eq!(
            long_version(),
            format!("{VERSION} ({GIT_SHA} {BUILD_DATE})")
        );
    }
}

#[test]
fn version_drift_warning_none_when_exe_cannot_be_run() {
    // A missing (or otherwise unspawnable) path is not a drift signal — nothing to compare
    // against, so stay silent rather than warn about a binary that plain isn't there.
    let missing =
        std::env::temp_dir().join(format!("moadim-no-such-binary-{}", uuid::Uuid::new_v4()));
    assert!(version_drift_warning(&missing, "moadim 0.20.0").is_none());
}

#[test]
fn version_drift_warning_none_when_on_disk_binary_exits_nonzero() {
    // A `--version` invocation that fails outright (crashed/misbuilt binary) gives no reliable
    // version string to compare, so this must not be treated as drift.
    let exe = write_script("moadim", "exit 1");
    assert!(version_drift_warning(&exe, "moadim 0.20.0").is_none());
    let _ = std::fs::remove_dir_all(exe.parent().unwrap());
}

#[test]
fn version_drift_warning_none_when_on_disk_binary_prints_nothing() {
    let exe = write_script("moadim", "exit 0");
    assert!(version_drift_warning(&exe, "moadim 0.20.0").is_none());
    let _ = std::fs::remove_dir_all(exe.parent().unwrap());
}

#[test]
fn version_drift_warning_none_when_versions_match() {
    let exe = write_script("moadim", "printf 'moadim 0.20.0'");
    assert!(version_drift_warning(&exe, "moadim 0.20.0").is_none());
    let _ = std::fs::remove_dir_all(exe.parent().unwrap());
}

#[test]
fn version_drift_warning_some_when_versions_differ() {
    // The on-disk binary reports a newer build than this process is running — the exact
    // scenario #167 describes: an un-restarted daemon silently keeps executing stale logic.
    let exe = write_script("moadim", "printf 'moadim 0.21.0'");
    let warning = version_drift_warning(&exe, "moadim 0.20.0").expect("expected drift warning");
    assert!(
        warning.contains("moadim 0.20.0") && warning.contains("moadim 0.21.0"),
        "expected both versions named in: {warning}"
    );
    assert!(
        warning.contains("moadim restart"),
        "expected a restart hint in: {warning}"
    );
    let _ = std::fs::remove_dir_all(exe.parent().unwrap());
}

#[test]
fn warn_on_drift_does_not_panic_when_versions_differ() {
    // Exercises the `Some(warning)` -> `log::warn!` branch; the periodic task in
    // `routes/http.rs` only ever calls this for its side effect.
    let exe = write_script("moadim", "printf 'moadim 0.21.0'");
    warn_on_drift(&exe, "moadim 0.20.0");
    let _ = std::fs::remove_dir_all(exe.parent().unwrap());
}

#[test]
fn warn_on_drift_does_not_panic_when_versions_match() {
    let exe = write_script("moadim", "printf 'moadim 0.20.0'");
    warn_on_drift(&exe, "moadim 0.20.0");
    let _ = std::fs::remove_dir_all(exe.parent().unwrap());
}
