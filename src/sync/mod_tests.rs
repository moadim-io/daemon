#![allow(clippy::missing_docs_in_private_items)]

use std::path::Path;

use super::*;

// ─── Crontab shim harness ───────────────────────────────────────────────────

/// A temp-dir crontab shim that emulates the system `crontab` binary against a
/// file-backed store, wired in via the `MOADIM_CRONTAB_BIN` env override.
///
/// `read_crontab`/`write_crontab` invoke the shim instead of the user's real
/// crontab. On drop it restores the previous `MOADIM_CRONTAB_BIN` value and
/// removes its temp directory.
struct CronShim {
    /// Temp directory holding the shim script and store file.
    base: std::path::PathBuf,
    /// Path to the file emulating the crontab contents.
    store_file: std::path::PathBuf,
    /// Saved prior value of `MOADIM_CRONTAB_BIN` to restore on drop.
    previous: Option<std::ffi::OsString>,
}

impl CronShim {
    /// Build a shim whose `-l` prints `initial` (or, when `None`, reports "no
    /// crontab" and exits 1) and whose `-` overwrites the store from stdin.
    fn new(initial: Option<&str>) -> Self {
        Self::with_body(initial, false)
    }

    /// Build a shim that always exits non-zero with a generic error on every
    /// invocation, emulating a crontab command failure.
    fn failing() -> Self {
        Self::with_body(None, true)
    }

    /// Internal constructor. When `always_fail` is set the shim exits 1 with a
    /// non-"no crontab" stderr for both `-l` and `-`.
    fn with_body(initial: Option<&str>, always_fail: bool) -> Self {
        use std::os::unix::fs::PermissionsExt;

        let base = std::env::temp_dir().join(format!("moadim-cronshim-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&base).unwrap();
        let store_file = base.join("store");
        // When `initial` is provided, seed the store file; when absent the shim's
        // `-l` will report "no crontab" because the file is missing.
        if let Some(content) = initial {
            std::fs::write(&store_file, content).unwrap();
        }

        let store_display = store_file.to_string_lossy().into_owned();
        let script_body = if always_fail {
            format!(
                "#!/bin/sh\nSTORE=\"{store_display}\"\nif [ \"$1\" = \"-\" ]; then cat > /dev/null; fi\necho \"crontab boom\" 1>&2\nexit 1\n"
            )
        } else {
            // `-l` prints the store (or reports "no crontab" when it is absent);
            // `-` overwrites the store from stdin.
            format!(
                "#!/bin/sh\nSTORE=\"{store_display}\"\nif [ \"$1\" = \"-l\" ]; then\n  if [ -f \"$STORE\" ]; then cat \"$STORE\"; else echo \"no crontab for tester\" 1>&2; exit 1; fi\nelif [ \"$1\" = \"-\" ]; then\n  cat > \"$STORE\"\nfi\n"
            )
        };

        let script_path = base.join("crontab-shim.sh");
        std::fs::write(&script_path, script_body).unwrap();
        std::fs::set_permissions(&script_path, std::fs::Permissions::from_mode(0o755)).unwrap();

        let previous = std::env::var_os("MOADIM_CRONTAB_BIN");
        // SAFETY: tests in this crate run single-threaded (RUST_TEST_THREADS=1);
        // the override is restored on drop.
        unsafe {
            std::env::set_var("MOADIM_CRONTAB_BIN", &script_path);
        }

        Self {
            base,
            store_file,
            previous,
        }
    }

    /// Build a shim whose `-l` reads `initial` normally but whose `-` always exits 1 without writing.
    ///
    /// Used to exercise error paths that only fire when `write_crontab` fails (e.g. the write step
    /// inside `clear_managed_crontab_blocks`).
    fn write_fails(initial: &str) -> Self {
        use std::os::unix::fs::PermissionsExt;

        let base = std::env::temp_dir().join(format!("moadim-cronshim-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&base).unwrap();
        let store_file = base.join("store");
        std::fs::write(&store_file, initial).unwrap();

        let store_display = store_file.to_string_lossy().into_owned();
        let script_body = format!(
            "#!/bin/sh\nSTORE=\"{store_display}\"\nif [ \"$1\" = \"-l\" ]; then\n  cat \"$STORE\"\nelif [ \"$1\" = \"-\" ]; then\n  cat > /dev/null\n  echo \"crontab write error\" 1>&2\n  exit 1\nfi\n"
        );

        let script_path = base.join("crontab-shim.sh");
        std::fs::write(&script_path, script_body).unwrap();
        std::fs::set_permissions(&script_path, std::fs::Permissions::from_mode(0o755)).unwrap();

        let previous = std::env::var_os("MOADIM_CRONTAB_BIN");
        // SAFETY: tests in this crate run single-threaded (RUST_TEST_THREADS=1);
        // the override is restored on drop.
        unsafe {
            std::env::set_var("MOADIM_CRONTAB_BIN", &script_path);
        }

        Self {
            base,
            store_file,
            previous,
        }
    }

    /// Read back the current emulated crontab contents from the store file.
    fn store_contents(&self) -> String {
        std::fs::read_to_string(&self.store_file).unwrap_or_default()
    }

    /// Build a shim whose `-` exits immediately *without reading stdin*,
    /// emulating a `crontab` that rejects input early and closes its end of
    /// the pipe mid-write. Used to exercise `write_crontab`'s write-failure
    /// (broken pipe) path, distinct from `write_fails` which drains stdin
    /// first and so never triggers a broken pipe.
    fn write_pipe_closed() -> Self {
        use std::os::unix::fs::PermissionsExt;

        let base = std::env::temp_dir().join(format!("moadim-cronshim-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&base).unwrap();
        let store_file = base.join("store");

        // No `cat` on the `-` branch: stdin is left unread and closed as soon
        // as the shim exits, so a large enough write from the parent
        // overflows the pipe buffer and observes a broken pipe.
        let script_body = "#!/bin/sh\nif [ \"$1\" = \"-\" ]; then exit 1; fi\n".to_string();

        let script_path = base.join("crontab-shim.sh");
        std::fs::write(&script_path, script_body).unwrap();
        std::fs::set_permissions(&script_path, std::fs::Permissions::from_mode(0o755)).unwrap();

        let previous = std::env::var_os("MOADIM_CRONTAB_BIN");
        // SAFETY: tests in this crate run single-threaded (RUST_TEST_THREADS=1);
        // the override is restored on drop.
        unsafe {
            std::env::set_var("MOADIM_CRONTAB_BIN", &script_path);
        }

        Self {
            base,
            store_file,
            previous,
        }
    }
}

impl Drop for CronShim {
    fn drop(&mut self) {
        // SAFETY: single-threaded test harness; restore the saved value.
        unsafe {
            match self.previous.take() {
                Some(value) => std::env::set_var("MOADIM_CRONTAB_BIN", value),
                None => std::env::remove_var("MOADIM_CRONTAB_BIN"),
            }
        }
        let _ = std::fs::remove_dir_all(&self.base);
    }
}

// ─── Schedule conversion ───────────────────────────────────────────────────

#[test]
fn to_os_schedule_7field_drops_sec_and_year() {
    assert_eq!(to_os_schedule("0 30 9 * * 1-5 *"), "30 9 * * 1-5");
}

#[test]
fn to_os_schedule_6field_drops_seconds() {
    // 6-field `sec min hour dom month dow` -> 5-field. Without reduction the
    // expression is written verbatim to the OS crontab and never fires.
    assert_eq!(to_os_schedule("0 */5 * * * *"), "*/5 * * * *");
    assert_eq!(to_os_schedule("30 0 9 * * 1-5"), "0 9 * * 1-5");
    assert_eq!(to_os_schedule("0 30 9 * * 1-5"), "30 9 * * 1-5");
    assert_eq!(to_os_schedule("*/30 * * * * *"), "* * * * *");
}

#[test]
fn to_os_schedule_passthrough_keyword() {
    assert_eq!(to_os_schedule("@daily"), "@daily");
    assert_eq!(to_os_schedule("@reboot"), "@reboot");
    assert_eq!(to_os_schedule("@hourly"), "@hourly");
}

#[test]
fn to_os_schedule_5field_unchanged() {
    assert_eq!(to_os_schedule("30 9 * * 1-5"), "30 9 * * 1-5");
}

#[test]
fn to_os_schedule_trims_whitespace() {
    assert_eq!(to_os_schedule("  0 0 * * * * *  "), "0 * * * *");
}

#[test]
fn to_os_schedule_non_5_or_7_field_passthrough() {
    // Covers the `_ =>` arm: neither @keyword, 5-field, nor 7-field.
    assert_eq!(to_os_schedule("1 2 3"), "1 2 3");
    assert_eq!(to_os_schedule("a b c d e f g h"), "a b c d e f g h");
}

// ─── SyncError Display & From<io::Error> ─────────────────────────────────────

#[test]
fn sync_error_display_renders_both_variants() {
    let cmd = SyncError::CrontabCommand("nope".to_string());
    assert_eq!(format!("{cmd}"), "crontab: nope");

    let io_err = std::io::Error::other("disk gone");
    let wrapped = SyncError::Io(io_err);
    assert_eq!(format!("{wrapped}"), "io: disk gone");
}

#[test]
fn sync_error_from_io_error_wraps_io_variant() {
    let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "denied");
    let converted: SyncError = io_err.into();
    match converted {
        SyncError::Io(inner) => {
            assert_eq!(inner.kind(), std::io::ErrorKind::PermissionDenied);
        }
        SyncError::CrontabCommand(msg) => panic!("expected Io variant, got CrontabCommand({msg})"),
    }
}

// ─── crontab_bin test-build guard ────────────────────────────────────────────

#[test]
fn crontab_bin_never_resolves_to_real_crontab_in_test_builds() {
    // Structural guard for issue #175: in a test build, with no `MOADIM_CRONTAB_BIN`
    // shim configured, `crontab_bin()` must never fall back to the real `crontab`,
    // so a test that forgets to isolate the crontab cannot clobber the developer's
    // live crontab. The resolved path must also not exist, so the eventual spawn
    // fails harmlessly and the sync only logs a warning.
    let saved = std::env::var_os("MOADIM_CRONTAB_BIN");
    // SAFETY: single-threaded test harness (RUST_TEST_THREADS=1); restored below.
    unsafe {
        std::env::remove_var("MOADIM_CRONTAB_BIN");
    }
    let bin = crontab_bin();
    unsafe {
        match saved {
            Some(value) => std::env::set_var("MOADIM_CRONTAB_BIN", value),
            None => std::env::remove_var("MOADIM_CRONTAB_BIN"),
        }
    }
    assert_ne!(
        bin, "crontab",
        "test build must not fall back to the real crontab"
    );
    assert!(
        !Path::new(&bin).exists(),
        "the test-build crontab guard path must not exist so the spawn fails: {bin}"
    );
}

// ─── read_crontab via shim ───────────────────────────────────────────────────

#[test]
fn read_crontab_returns_store_contents_on_success() {
    let shim = CronShim::new(Some("0 0 * * * /bin/existing\n"));
    let result = read_crontab().unwrap();
    assert_eq!(result, "0 0 * * * /bin/existing\n");
    drop(shim);
}

#[test]
fn read_crontab_empty_when_no_crontab() {
    // Shim with no store file reports "no crontab" and exits 1 → empty string.
    let shim = CronShim::new(None);
    let result = read_crontab().unwrap();
    assert_eq!(result, "");
    drop(shim);
}

#[test]
fn read_crontab_errors_on_non_no_crontab_failure() {
    // Shim exits non-zero with stderr that does NOT contain "no crontab".
    let shim = CronShim::failing();
    let err = read_crontab().unwrap_err();
    match err {
        SyncError::CrontabCommand(msg) => assert!(msg.contains("boom"), "unexpected msg: {msg}"),
        SyncError::Io(io) => panic!("expected CrontabCommand, got Io({io})"),
    }
    drop(shim);
}

// ─── write_crontab via shim ──────────────────────────────────────────────────

#[test]
fn write_crontab_persists_content_on_success() {
    let shim = CronShim::new(Some(""));
    write_crontab("hello # moadim-routine:z\n").unwrap();
    assert_eq!(shim.store_contents(), "hello # moadim-routine:z\n");
    drop(shim);
}

#[test]
fn write_crontab_errors_on_non_success_exit() {
    let shim = CronShim::failing();
    let err = write_crontab("anything\n").unwrap_err();
    match err {
        SyncError::CrontabCommand(msg) => assert!(
            msg.contains("exited with"),
            "expected exit message, got: {msg}"
        ),
        SyncError::Io(io) => panic!("expected CrontabCommand, got Io({io})"),
    }
    drop(shim);
}

#[test]
fn write_crontab_errors_instead_of_panicking_on_broken_pipe() {
    // The shim's `-` branch never reads stdin and exits immediately, so
    // writing content larger than the OS pipe buffer must observe a broken
    // pipe. Before this test, that write failure was `.expect()`'d into a
    // panic instead of being returned as a `SyncError`.
    let shim = CronShim::write_pipe_closed();
    let big_content = "x".repeat(4 * 1024 * 1024);
    let err = write_crontab(&big_content).unwrap_err();
    match err {
        SyncError::Io(_) => {}
        SyncError::CrontabCommand(msg) => {
            panic!("expected Io error from the broken pipe, got CrontabCommand({msg})")
        }
    }
    drop(shim);
}

#[test]
fn write_crontab_errors_when_binary_is_missing() {
    // Pointing the crontab seam at a nonexistent binary makes the spawn fail, exercising the
    // spawn-failure error branch.
    let previous = std::env::var_os("MOADIM_CRONTAB_BIN");
    // SAFETY: single-threaded test execution.
    unsafe {
        std::env::set_var(
            "MOADIM_CRONTAB_BIN",
            "/nonexistent/moadim-no-such-crontab-xyz",
        );
    }
    let result = write_crontab("# BEGIN MOADIM-ROUTINES\n# END MOADIM-ROUTINES\n");
    // SAFETY: single-threaded test execution.
    unsafe {
        match previous {
            Some(value) => std::env::set_var("MOADIM_CRONTAB_BIN", value),
            None => std::env::remove_var("MOADIM_CRONTAB_BIN"),
        }
    }
    assert!(
        result.is_err(),
        "spawning a missing crontab binary must error"
    );
}

// ─── replace_block_with ──────────────────────────────────────────────────────

const TEST_BEGIN: &str = "# BEGIN TEST";
const TEST_END: &str = "# END TEST";

#[test]
fn replace_block_with_inserts_when_absent() {
    let crontab = "0 * * * * /existing\n";
    let block = "# BEGIN TEST\n# hdr\n# END TEST";
    let result = replace_block_with(crontab, block, TEST_BEGIN, TEST_END);
    assert!(result.contains(TEST_BEGIN));
    assert!(result.contains(TEST_END));
    assert!(result.contains("/existing"));
}

#[test]
fn replace_block_with_replaces_existing() {
    let crontab = "before\n# BEGIN TEST\nold line # tag:old\n# END TEST\nafter\n";
    let block = "# BEGIN TEST\nnew line # tag:new\n# END TEST";
    let result = replace_block_with(crontab, block, TEST_BEGIN, TEST_END);
    assert!(result.contains("new line"), "new line missing: {result}");
    assert!(
        !result.contains("old line"),
        "old line still present: {result}"
    );
    assert!(result.contains("before"), "before missing: {result}");
    assert!(result.contains("after"), "after missing: {result}");
}

#[test]
fn replace_block_with_idempotent() {
    let block = "# BEGIN TEST\n# hdr\n30 9 * * * /cmd # tag:uid\n# END TEST";
    let crontab = format!("{block}\n");
    let result = replace_block_with(&crontab, block, TEST_BEGIN, TEST_END);
    assert!(result.contains("30 9 * * * /cmd # tag:uid"));
}

#[test]
fn replace_block_with_handles_malformed_missing_end() {
    let crontab = "pre\n# BEGIN TEST\norphan line\n";
    let block = "# BEGIN TEST\n# hdr\n# END TEST";
    let result = replace_block_with(crontab, block, TEST_BEGIN, TEST_END);
    assert!(result.contains(TEST_END), "end marker missing: {result}");
    assert!(
        !result.contains("orphan"),
        "orphan line still present: {result}"
    );
    assert!(result.contains("pre"), "pre-content missing: {result}");
}

#[test]
fn replace_block_with_empty_crontab() {
    let block = "# BEGIN TEST\n# hdr\n# END TEST";
    let result = replace_block_with("", block, TEST_BEGIN, TEST_END);
    assert_eq!(result.trim(), block.trim());
}

#[test]
fn replace_block_with_appends_trailing_newline_to_unterminated_rest() {
    // Covers the `if !result.ends_with('\n')` branch: content follows the END
    // marker but does not end in a newline, so one is appended to preserve it.
    let crontab = "# BEGIN TEST\nold # tag:x\n# END TEST\ntrailing line no newline";
    let block = "# BEGIN TEST\nnew # tag:y\n# END TEST";
    let result = replace_block_with(crontab, block, TEST_BEGIN, TEST_END);
    assert!(
        result.contains("new # tag:y"),
        "block not replaced: {result}"
    );
    assert!(
        result.contains("trailing line no newline"),
        "trailing content lost: {result}"
    );
    assert!(
        result.ends_with('\n'),
        "trailing newline not appended: {result:?}"
    );
}

// ─── clear_managed_crontab_blocks (moadim uninstall, #380) ───────────────────

/// A user crontab line preceding the managed block.
const USER_BEFORE: &str = "0 0 * * * /usr/bin/backup";
/// A user crontab line following the managed block.
const USER_AFTER: &str = "0 9 * * * /usr/bin/report";

/// A crontab carrying the managed routines block wrapped by two unmanaged user
/// entries, mirroring a real install.
fn crontab_with_routines_block() -> String {
    format!(
        "{USER_BEFORE}\n\
         # BEGIN MOADIM-ROUTINES\n\
         # Managed by moadim — routines (agent tmux sessions)\n\
         */5 * * * * /bin/sh -l '/x/run.sh' # moadim-routine:r1\n\
         # END MOADIM-ROUTINES\n\
         {USER_AFTER}\n"
    )
}

#[test]
fn clear_removes_the_block_and_preserves_user_entries() {
    let shim = CronShim::new(Some(&crontab_with_routines_block()));
    let removed = clear_managed_crontab_blocks().unwrap();
    assert_eq!(removed, 1, "one routine line");
    let after = shim.store_contents();
    assert!(!after.contains("# BEGIN MOADIM-ROUTINES"));
    assert!(!after.contains("# moadim-routine:"));
    assert!(
        after.contains(USER_BEFORE),
        "user entry before the block survives"
    );
    assert!(
        after.contains(USER_AFTER),
        "user entry after the block survives"
    );
}

#[test]
fn clear_is_idempotent_on_an_already_clean_crontab() {
    let shim = CronShim::new(Some(&crontab_with_routines_block()));
    assert_eq!(clear_managed_crontab_blocks().unwrap(), 1);
    let after_first = shim.store_contents();
    // A second uninstall has nothing managed to remove: returns 0 and leaves the
    // crontab byte-for-byte unchanged (no spurious rewrite).
    assert_eq!(clear_managed_crontab_blocks().unwrap(), 0);
    assert_eq!(shim.store_contents(), after_first);
}

#[test]
fn clear_on_a_crontab_without_managed_blocks_is_a_noop() {
    let plain = format!("{USER_BEFORE}\n{USER_AFTER}\n");
    let shim = CronShim::new(Some(&plain));
    assert_eq!(clear_managed_crontab_blocks().unwrap(), 0);
    assert_eq!(shim.store_contents(), plain, "untouched");
}

#[test]
fn clear_on_an_absent_crontab_succeeds_with_zero() {
    let shim = CronShim::new(None);
    assert_eq!(clear_managed_crontab_blocks().unwrap(), 0);
    assert_eq!(shim.store_contents(), "", "nothing was written");
}

#[test]
fn clear_managed_crontab_blocks_errors_on_read_failure() {
    // A failing shim makes `read_crontab` return Err, exercising the `?` early on.
    let _shim = CronShim::failing();
    let err = clear_managed_crontab_blocks().unwrap_err();
    match err {
        SyncError::CrontabCommand(msg) => assert!(msg.contains("boom"), "unexpected msg: {msg}"),
        SyncError::Io(io) => panic!("expected CrontabCommand, got Io({io})"),
    }
}

#[test]
fn clear_managed_crontab_blocks_errors_on_write_failure() {
    // The initial crontab contains the managed block, so `updated != current` after removal and
    // `write_crontab` is called. The write-failing shim makes that call return Err.
    let initial =
        "# BEGIN MOADIM-ROUTINES\n* * * * * /x # moadim-routine:r1\n# END MOADIM-ROUTINES\n";
    let _shim = CronShim::write_fails(initial);
    let err = clear_managed_crontab_blocks().unwrap_err();
    assert!(
        matches!(err, SyncError::CrontabCommand(_)),
        "expected CrontabCommand error from write failure, got: {err:?}"
    );
}
