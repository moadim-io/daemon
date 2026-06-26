//! Tests for [`super`] build-provenance formatting.

use super::{format_version, long_version, BUILD_DATE, GIT_SHA, VERSION};

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
