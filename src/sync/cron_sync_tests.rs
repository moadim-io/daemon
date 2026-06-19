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

    /// Read back the current emulated crontab contents from the store file.
    fn store_contents(&self) -> String {
        std::fs::read_to_string(&self.store_file).unwrap_or_default()
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

/// Build a managed [`CronJob`] with an explicit `created_at` for ordering tests.
fn make_managed_job(id: &str, schedule: &str, handler: &str, created_at: u64) -> CronJob {
    CronJob {
        id: id.to_string(),
        schedule: schedule.to_string(),
        handler: handler.to_string(),
        metadata: serde_json::json!({}),
        enabled: true,
        source: "managed".to_string(),
        created_at,
        updated_at: created_at,
        last_manual_trigger_at: None,
    }
}

/// Wrap `jobs` into a fresh [`CronStore`].
fn store_with(jobs: Vec<CronJob>) -> CronStore {
    let map: HashMap<String, CronJob> = jobs.into_iter().map(|j| (j.id.clone(), j)).collect();
    std::sync::Arc::new(std::sync::Mutex::new(map))
}

// ─── Schedule conversion ───────────────────────────────────────────────────

#[test]
fn to_os_schedule_7field_drops_sec_and_year() {
    assert_eq!(to_os_schedule("0 30 9 * * 1-5 *"), "30 9 * * 1-5");
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
fn to_moadim_schedule_5field_passthrough() {
    assert_eq!(to_moadim_schedule("30 9 * * 1-5"), "30 9 * * 1-5");
}

#[test]
fn to_moadim_schedule_passthrough_keyword() {
    assert_eq!(to_moadim_schedule("@daily"), "@daily");
    assert_eq!(to_moadim_schedule("@hourly"), "@hourly");
}

#[test]
fn to_moadim_schedule_trims_whitespace() {
    assert_eq!(to_moadim_schedule("  30 9 * * 1-5  "), "30 9 * * 1-5");
}

#[test]
fn roundtrip_5field_via_os() {
    let moadim = "30 9 * * 1-5";
    let os = to_os_schedule(moadim);
    assert_eq!(to_moadim_schedule(&os), "30 9 * * 1-5");
}

#[test]
fn roundtrip_keyword_unchanged() {
    let moadim = "@weekly";
    assert_eq!(to_moadim_schedule(&to_os_schedule(moadim)), "@weekly");
}

// ─── Handler path resolution ───────────────────────────────────────────────

#[test]
fn resolve_handler_path_returns_name_when_not_found() {
    let dir = Path::new("/nonexistent/handlers");
    let result = resolve_handler_path("my-script", dir);
    assert_eq!(result, dir.join("my-script"));
}

#[test]
fn handler_from_command_strips_prefix_and_extension() {
    let dir = Path::new("/home/user/.config/moadim/handlers");
    let cmd = "/home/user/.config/moadim/handlers/send-report.sh";
    assert_eq!(handler_from_command(cmd, dir), "send-report");
}

#[test]
fn handler_from_command_no_extension() {
    let dir = Path::new("/home/user/.config/moadim/handlers");
    let cmd = "/home/user/.config/moadim/handlers/cleanup";
    assert_eq!(handler_from_command(cmd, dir), "cleanup");
}

#[test]
fn handler_from_command_outside_dir_uses_stem() {
    let dir = Path::new("/home/user/.config/moadim/handlers");
    let cmd = "/usr/local/bin/my-script.sh";
    assert_eq!(handler_from_command(cmd, dir), "my-script");
}

// ─── format_crontab_line ───────────────────────────────────────────────────

fn make_job(id: &str, schedule: &str, handler: &str) -> CronJob {
    CronJob {
        id: id.to_string(),
        schedule: schedule.to_string(),
        handler: handler.to_string(),
        metadata: serde_json::json!({}),
        enabled: true,
        source: "managed".to_string(),
        created_at: 0,
        updated_at: 0,
        last_manual_trigger_at: None,
    }
}

#[test]
fn format_crontab_line_produces_moadim_tag() {
    let job = make_job("abc-123", "30 9 * * 1-5", "send-report");
    let dir = Path::new("/home/user/.config/moadim/handlers");
    let line = format_crontab_line(&job, dir);
    assert!(line.contains("# moadim:abc-123"), "missing tag: {line}");
    assert!(line.starts_with("30 9 * * 1-5"), "wrong schedule: {line}");
    assert!(line.contains("send-report"), "missing handler: {line}");
}

#[test]
fn format_crontab_line_keyword_passthrough() {
    let job = make_job("uid-1", "@daily", "backup");
    let dir = Path::new("/home/user/.config/moadim/handlers");
    let line = format_crontab_line(&job, dir);
    assert!(line.starts_with("@daily"), "wrong schedule: {line}");
}

// ─── parse_moadim_line ─────────────────────────────────────────────────────

#[test]
fn parse_moadim_line_standard() {
    let line = "30 9 * * 1-5 /home/user/.config/moadim/handlers/send-report # moadim:abc-123";
    let (id, sched, cmd) = parse_moadim_line(line).unwrap();
    assert_eq!(id, "abc-123");
    assert_eq!(sched, "30 9 * * 1-5");
    assert_eq!(cmd, "/home/user/.config/moadim/handlers/send-report");
}

#[test]
fn parse_moadim_line_keyword() {
    let line = "@daily /home/user/.config/moadim/handlers/backup # moadim:xyz-789";
    let (id, sched, cmd) = parse_moadim_line(line).unwrap();
    assert_eq!(id, "xyz-789");
    assert_eq!(sched, "@daily");
    assert_eq!(cmd, "/home/user/.config/moadim/handlers/backup");
}

#[test]
fn parse_moadim_line_no_tag_returns_none() {
    assert!(parse_moadim_line("30 9 * * 1-5 /usr/bin/cmd").is_none());
}

#[test]
fn parse_moadim_line_empty_uuid_returns_none() {
    assert!(parse_moadim_line("30 9 * * 1-5 /cmd # moadim:").is_none());
}

#[test]
fn parse_moadim_line_too_few_fields_returns_none() {
    assert!(parse_moadim_line("30 9 /cmd # moadim:uid").is_none());
}

// ─── parse_block ───────────────────────────────────────────────────────────

#[test]
fn parse_block_extracts_entries() {
    let crontab = "
0 0 * * * /bin/other
# BEGIN MOADIM
# Managed by moadim
30 9 * * 1-5 /handlers/send # moadim:uuid-1
0 0 * * 0 /handlers/clean # moadim:uuid-2
# END MOADIM
@reboot /bin/startup
";
    let entries = parse_block(crontab);
    assert_eq!(entries.len(), 2);
    assert!(entries.contains_key("uuid-1"));
    assert!(entries.contains_key("uuid-2"));
}

#[test]
fn parse_block_empty_when_no_block() {
    assert!(parse_block("0 * * * * /bin/cmd").is_empty());
}

#[test]
fn parse_block_ignores_comment_lines_inside_block() {
    let crontab = "# BEGIN MOADIM\n# a comment\n30 9 * * * /cmd # moadim:id1\n# END MOADIM\n";
    let entries = parse_block(crontab);
    assert_eq!(entries.len(), 1);
    assert!(entries.contains_key("id1"));
}

// ─── replace_block ─────────────────────────────────────────────────────────

#[test]
fn replace_block_inserts_when_absent() {
    let crontab = "0 * * * * /existing\n";
    let block = "# BEGIN MOADIM\n# hdr\n# END MOADIM";
    let result = replace_block(crontab, block);
    assert!(result.contains(BLOCK_BEGIN));
    assert!(result.contains("# END MOADIM"));
    assert!(result.contains("/existing"));
}

#[test]
fn replace_block_replaces_existing() {
    let crontab = "before\n# BEGIN MOADIM\nold line # moadim:old\n# END MOADIM\nafter\n";
    let block = "# BEGIN MOADIM\nnew line # moadim:new\n# END MOADIM";
    let result = replace_block(crontab, block);
    assert!(result.contains("new line"), "new line missing: {result}");
    assert!(
        !result.contains("old line"),
        "old line still present: {result}"
    );
    assert!(result.contains("before"), "before missing: {result}");
    assert!(result.contains("after"), "after missing: {result}");
}

#[test]
fn replace_block_idempotent() {
    let block = "# BEGIN MOADIM\n# hdr\n30 9 * * * /cmd # moadim:uid\n# END MOADIM";
    let crontab = format!("{block}\n");
    let result = replace_block(&crontab, block);
    // Content identical (modulo trailing newline normalisation)
    assert!(result.contains("30 9 * * * /cmd # moadim:uid"));
}

#[test]
fn replace_block_handles_malformed_missing_end() {
    let crontab = "pre\n# BEGIN MOADIM\norphan line\n";
    let block = "# BEGIN MOADIM\n# hdr\n# END MOADIM";
    let result = replace_block(crontab, block);
    assert!(
        result.contains("# END MOADIM"),
        "end marker missing: {result}"
    );
    assert!(
        !result.contains("orphan"),
        "orphan line still present: {result}"
    );
    assert!(result.contains("pre"), "pre-content missing: {result}");
}

#[test]
fn replace_block_empty_crontab() {
    let block = "# BEGIN MOADIM\n# hdr\n# END MOADIM";
    let result = replace_block("", block);
    assert_eq!(result.trim(), block.trim());
}

#[test]
fn replace_block_appends_trailing_newline_to_unterminated_rest() {
    // Covers the `if !result.ends_with('\n')` branch: content follows the END
    // marker but does not end in a newline, so one is appended to preserve it.
    let crontab = "# BEGIN MOADIM\nold # moadim:x\n# END MOADIM\ntrailing line no newline";
    let block = "# BEGIN MOADIM\nnew # moadim:y\n# END MOADIM";
    let result = replace_block(crontab, block);
    assert!(
        result.contains("new # moadim:y"),
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

// ─── marker collision with the routines block (issue #324) ───────────────────

#[test]
fn replace_block_does_not_match_routines_marker_as_prefix() {
    // A crontab that holds ONLY the routines block (`# BEGIN MOADIM-ROUTINES`)
    // and no cron-jobs block. The cron-jobs marker `# BEGIN MOADIM` is a prefix
    // of the routines marker; a substring `find` would match it here and wipe
    // the routines block. Whole-line matching must leave the routines block
    // untouched and *append* the cron-jobs block instead.
    let crontab = "# BEGIN MOADIM-ROUTINES\n\
                   # Managed by moadim — routines (agent tmux sessions)\n\
                   * * * * * /bin/sh -l '/r/run.sh' # moadim-routine:rid\n\
                   # END MOADIM-ROUTINES\n";
    let block = "# BEGIN MOADIM\n# hdr\n30 9 * * * /cmd # moadim:uid\n# END MOADIM";
    let result = replace_block(crontab, block);

    assert!(
        result.contains("# moadim-routine:rid"),
        "routines block was wiped: {result}"
    );
    assert!(
        result.contains("# BEGIN MOADIM-ROUTINES"),
        "routines begin marker lost: {result}"
    );
    assert!(
        result.contains("30 9 * * * /cmd # moadim:uid"),
        "cron-jobs block not appended: {result}"
    );
}

#[test]
fn replace_block_targets_exact_marker_among_both_blocks() {
    // Both blocks present. Replacing the cron-jobs block must edit only it and
    // leave the adjacent routines block byte-for-byte intact.
    let crontab = "# BEGIN MOADIM\nold # moadim:old\n# END MOADIM\n\
                   # BEGIN MOADIM-ROUTINES\n* * * * * /bin/sh -l '/r/run.sh' # moadim-routine:rid\n# END MOADIM-ROUTINES\n";
    let block = "# BEGIN MOADIM\nnew # moadim:new\n# END MOADIM";
    let result = replace_block(crontab, block);

    assert!(
        result.contains("new # moadim:new"),
        "not replaced: {result}"
    );
    assert!(
        !result.contains("old # moadim:old"),
        "stale line kept: {result}"
    );
    assert!(
        result.contains("* * * * * /bin/sh -l '/r/run.sh' # moadim-routine:rid"),
        "routines block disturbed: {result}"
    );
    assert!(
        result.contains("# END MOADIM-ROUTINES"),
        "routines end marker lost: {result}"
    );
}

#[test]
fn find_marker_line_ignores_surrounding_whitespace() {
    // A marker indented with leading/trailing whitespace still matches by line,
    // and the reported offsets bracket the marker text exactly.
    let crontab = "noise\n  # END MOADIM  \n";
    let (start, end) = find_marker_line(crontab, "# END MOADIM").expect("marker found");
    assert_eq!(&crontab[start..end], "  # END MOADIM");
    assert!(find_marker_line(crontab, "# BEGIN MOADIM").is_none());
}

// ─── to_os_schedule odd-field branch ─────────────────────────────────────────

#[test]
fn to_os_schedule_non_5_or_7_field_passthrough() {
    // Covers the `_ =>` arm: neither @keyword, 5-field, nor 7-field.
    assert_eq!(to_os_schedule("1 2 3"), "1 2 3");
    assert_eq!(to_os_schedule("a b c d e f g h"), "a b c d e f g h");
}

// ─── resolve_handler_path extension match ────────────────────────────────────

#[test]
fn resolve_handler_path_matches_extension() {
    // Covers the extension-candidate `return candidate` branch: an exact match
    // is absent but `<handler>.sh` exists in the directory.
    let dir = std::env::temp_dir().join(format!("moadim-handlers-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let script = dir.join("greet.sh");
    std::fs::write(&script, "#!/bin/sh\n").unwrap();

    let resolved = resolve_handler_path("greet", &dir);
    assert_eq!(resolved, script);

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn resolve_handler_path_matches_exact() {
    // Covers the exact-match `return exact` branch: a file named exactly like the
    // handler (no extension) exists in the directory.
    let dir = std::env::temp_dir().join(format!("moadim-handlers-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let script = dir.join("deploy");
    std::fs::write(&script, "#!/bin/sh\n").unwrap();

    let resolved = resolve_handler_path("deploy", &dir);
    assert_eq!(resolved, script);

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn build_block_empty_store_emits_header_only() {
    // Covers the `lines.is_empty()` branch of build_block: an empty store yields the
    // begin/header/end markers with no managed job lines between them.
    let block = build_block(&store_with(vec![]));
    assert_eq!(block, format!("{BLOCK_BEGIN}\n{BLOCK_HEADER}\n{BLOCK_END}"));
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
    write_crontab("hello # moadim:z\n").unwrap();
    assert_eq!(shim.store_contents(), "hello # moadim:z\n");
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

// ─── sync_to_crontab ─────────────────────────────────────────────────────────

#[test]
fn sync_to_crontab_writes_block_and_is_idempotent() {
    let shim = CronShim::new(Some(""));
    let store = store_with(vec![make_managed_job(
        "job-1",
        "30 9 * * 1-5",
        "send-report",
        1,
    )]);

    // First sync installs the block.
    sync_to_crontab(&store).unwrap();
    let after_first = shim.store_contents();
    assert!(
        after_first.contains("# moadim:job-1"),
        "block missing: {after_first}"
    );

    // Second sync makes no change: covers the `new_crontab == current` early return.
    sync_to_crontab(&store).unwrap();
    assert_eq!(
        shim.store_contents(),
        after_first,
        "idempotent sync changed the crontab"
    );
    drop(shim);
}

// ─── sync_from_crontab ───────────────────────────────────────────────────────

#[test]
fn sync_from_crontab_updates_existing_and_imports_unknown() {
    // Block carries: an updated schedule for a known job, and an unknown UUID to import.
    let crontab = "\
# BEGIN MOADIM
# Managed by moadim
0 6 * * * /handlers/known # moadim:known-1
15 3 * * 0 /handlers/fresh.sh # moadim:imported-9
# END MOADIM
";
    let shim = CronShim::new(Some(crontab));
    let store = store_with(vec![make_managed_job(
        "known-1",
        "30 9 * * 1-5",
        "known",
        1,
    )]);

    let changed = sync_from_crontab(&store).unwrap();
    assert!(changed, "expected changes to be reported");

    let lock = store.lock().unwrap();
    let known = lock.get("known-1").unwrap();
    assert_eq!(known.schedule, "0 6 * * *", "schedule not updated");

    let imported = lock.get("imported-9").expect("unknown UUID not imported");
    assert_eq!(imported.schedule, "15 3 * * 0");
    assert_eq!(imported.handler, "fresh");
    assert_eq!(imported.source, "managed");
    assert!(imported.enabled);
    drop(lock);
    drop(shim);

    // sync_from_crontab persists updated/imported jobs to disk; clean them up.
    let _ = std::fs::remove_dir_all(crate::paths::job_dir("known-1"));
    let _ = std::fs::remove_dir_all(crate::paths::job_dir("imported-9"));
}

#[test]
fn sync_from_crontab_updates_handler_only() {
    // The block changes a known job's handler but keeps its schedule, exercising the
    // `if handler_changed { job.handler = new_handler; }` branch independently of the schedule one.
    let crontab = "\
# BEGIN MOADIM
# Managed by moadim
30 9 * * 1-5 /handlers/renamed # moadim:hjob-1
# END MOADIM
";
    let shim = CronShim::new(Some(crontab));
    let store = store_with(vec![make_managed_job(
        "hjob-1",
        "30 9 * * 1-5",
        "original",
        1,
    )]);

    let changed = sync_from_crontab(&store).unwrap();
    assert!(changed, "expected the handler change to be reported");

    let lock = store.lock().unwrap();
    let job = lock.get("hjob-1").unwrap();
    assert_eq!(job.schedule, "30 9 * * 1-5", "schedule must be unchanged");
    assert_eq!(job.handler, "renamed", "handler should be updated");
    drop(lock);
    drop(shim);

    let _ = std::fs::remove_dir_all(crate::paths::job_dir("hjob-1"));
}

#[test]
fn sync_from_crontab_logs_when_write_job_fails() {
    // Import path runs (unknown UUID) but `write_job` fails because a regular file occupies the
    // jobs directory path under the override home, so `create_dir_all` errors. This exercises the
    // `log::warn!("cron_sync: failed to persist job ...")` branch while leaving the in-memory store
    // updated (changed == true).
    let home = std::env::temp_dir().join(format!("moadim-cronpersist-{}", uuid::Uuid::new_v4()));
    let previous_home = std::env::var_os("MOADIM_HOME_OVERRIDE");
    // SAFETY: single-threaded test harness; restored below.
    unsafe {
        std::env::set_var("MOADIM_HOME_OVERRIDE", &home);
    }
    // Block the jobs dir: create `{home}/.config/moadim` and drop a regular file named `jobs`, so
    // `job_dir(id)` -> `.../jobs/<id>` cannot be created.
    let moadim_cfg = home.join(".config").join("moadim");
    std::fs::create_dir_all(&moadim_cfg).unwrap();
    std::fs::write(moadim_cfg.join("jobs"), "block the jobs dir").unwrap();

    let crontab = "\
# BEGIN MOADIM
# Managed by moadim
15 3 * * 0 /handlers/fresh.sh # moadim:persist-fail-9
# END MOADIM
";
    let shim = CronShim::new(Some(crontab));
    let store = store_with(vec![]);

    let changed = sync_from_crontab(&store).unwrap();
    assert!(
        changed,
        "import still flips changed even though persistence failed"
    );
    assert!(
        store.lock().unwrap().contains_key("persist-fail-9"),
        "imported job is in the in-memory store regardless of the persist failure"
    );

    drop(shim);
    // SAFETY: single-threaded harness; restore the saved override.
    unsafe {
        match previous_home {
            Some(value) => std::env::set_var("MOADIM_HOME_OVERRIDE", value),
            None => std::env::remove_var("MOADIM_HOME_OVERRIDE"),
        }
    }
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn sync_from_crontab_no_change_returns_false() {
    // Block matches the store exactly → no updates, no imports.
    let crontab = "\
# BEGIN MOADIM
# Managed by moadim
30 9 * * 1-5 /handlers/known # moadim:known-1
# END MOADIM
";
    let shim = CronShim::new(Some(crontab));
    let store = store_with(vec![make_managed_job(
        "known-1",
        "30 9 * * 1-5",
        "known",
        1,
    )]);

    let changed = sync_from_crontab(&store).unwrap();
    assert!(!changed, "expected no changes");
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
    let result = write_crontab("# BEGIN MOADIM\n# END MOADIM\n");
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

#[test]
fn handler_from_command_strips_dir_prefix_to_stem() {
    let dir = std::path::Path::new("/handlers");
    // A command under `dir` resolves via the stripped relative stem (the strip_prefix Ok branch).
    assert_eq!(handler_from_command("/handlers/report.sh", dir), "report");
    // A bare command falls back to its own file stem (the strip_prefix Err branch).
    assert_eq!(handler_from_command("standalone.py", dir), "standalone");
    // A command with no file stem falls back to the trimmed command (the unwrap_or_else branch).
    assert_eq!(handler_from_command("/", dir), "/");
}

#[test]
fn sync_from_crontab_skips_managed_job_absent_from_block() {
    // A managed job lives in the store but the crontab block is empty (the job was disabled, so
    // the last forward sync excluded it). The reconcile loop must skip it (the `if let Some` miss)
    // rather than touch it, and report no change.
    let crontab = "\
# BEGIN MOADIM
# Managed by moadim
# END MOADIM
";
    let shim = CronShim::new(Some(crontab));
    let store = store_with(vec![make_managed_job(
        "absent-from-block-1",
        "30 9 * * 1-5",
        "ghost",
        1,
    )]);

    let changed = sync_from_crontab(&store).unwrap();
    assert!(
        !changed,
        "a store job missing from the block is left untouched"
    );
    assert!(
        store.lock().unwrap().contains_key("absent-from-block-1"),
        "the absent job stays in the store"
    );
    drop(shim);
}

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
