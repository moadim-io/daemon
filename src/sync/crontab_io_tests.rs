#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

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

    /// Build a shim whose `-` branch drains stdin and then hangs forever.
    fn write_hangs() -> Self {
        use std::os::unix::fs::PermissionsExt;

        let base = std::env::temp_dir().join(format!("moadim-cronshim-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&base).unwrap();
        let store_file = base.join("store");
        let script_body = "#!/bin/sh\nif [ \"$1\" = \"-\" ]; then cat > /dev/null; sleep 30; fi\n";
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

/// Environment variable guard for tests that override timeout settings.
struct EnvGuard {
    /// Variable name to restore.
    name: &'static str,
    /// Previous variable value.
    previous: Option<std::ffi::OsString>,
}

impl EnvGuard {
    /// Set `name` to `value` until this guard is dropped.
    fn set(name: &'static str, value: &str) -> Self {
        let previous = std::env::var_os(name);
        // SAFETY: tests in this crate run single-threaded (RUST_TEST_THREADS=1).
        unsafe {
            std::env::set_var(name, value);
        }
        Self { name, previous }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        // SAFETY: tests in this crate run single-threaded (RUST_TEST_THREADS=1).
        unsafe {
            match self.previous.take() {
                Some(value) => std::env::set_var(self.name, value),
                None => std::env::remove_var(self.name),
            }
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
fn write_crontab_times_out_and_kills_hung_install() {
    let _timeout = EnvGuard::set(CRONTAB_WRITE_TIMEOUT_ENV, "1");
    let shim = CronShim::write_hangs();
    let err = write_crontab("anything\n").unwrap_err();
    match err {
        SyncError::CrontabCommand(msg) => {
            assert!(msg.contains("timed out after 1s"), "unexpected msg: {msg}");
            assert!(msg.contains("killed pid"), "missing kill detail: {msg}");
        }
        SyncError::Io(io) => panic!("expected CrontabCommand, got Io({io})"),
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
