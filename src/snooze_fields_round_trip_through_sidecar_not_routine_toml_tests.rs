#![allow(
    clippy::wildcard_imports,
    clippy::too_many_arguments,
    clippy::needless_pass_by_value,
    dead_code,
    reason = "split-out module preserves existing code while keeping files under the linecheck limit"
)]
use super::*;

#[test]
fn snooze_fields_round_trip_through_sidecar_not_routine_toml() {
    // Snooze state is ephemeral/daemon-owned, like last_manual_trigger_at: it lives in the
    // gitignored state.local.toml sidecar, not the tracked routine.toml, and round-trips on load.
    with_override_home(|_home| {
        let title = "Rs Snooze Sidecar Routine";
        let slug = slugify(title);
        let mut routine = make_routine("rs-snooze-sidecar-id", title);
        routine.snoozed_until = Some(999_999);
        write_routine(&routine).unwrap();

        let toml_text = std::fs::read_to_string(crate::paths::routine_toml_path(&slug)).unwrap();
        assert!(
            !toml_text.contains("snoozed_until"),
            "routine.toml must not carry snooze state: {toml_text}"
        );
        let state_text = std::fs::read_to_string(crate::paths::routine_state_path(&slug)).unwrap();
        assert!(state_text.contains("snoozed_until"));

        let loaded = load_routine_from_dir(&slug).unwrap();
        assert_eq!(loaded.snoozed_until, Some(999_999));
        assert_eq!(loaded.skip_runs, None);
    });
}

#[test]
fn skip_runs_round_trips_and_clearing_both_removes_sidecar() {
    with_override_home(|_home| {
        let title = "Rs Skip Runs Sidecar Routine";
        let slug = slugify(title);
        let mut routine = make_routine("rs-skip-runs-sidecar-id", title);
        routine.skip_runs = Some(3);
        write_routine(&routine).unwrap();
        assert!(crate::paths::routine_state_path(&slug).exists());
        assert_eq!(load_routine_from_dir(&slug).unwrap().skip_runs, Some(3));

        routine.skip_runs = None;
        write_routine(&routine).unwrap();
        assert!(
            !crate::paths::routine_state_path(&slug).exists(),
            "sidecar should be removed once no runtime state (trigger or snooze) remains"
        );
        assert_eq!(load_routine_from_dir(&slug).unwrap().skip_runs, None);
    });
}

#[test]
fn append_manual_trigger_log_creates_and_appends() {
    // Each call appends one timestamp line; the log grows and load reads the last line.
    with_override_home(|_home| {
        let title = "Rs Manual Log Append Routine";
        let slug = slugify(title);
        write_routine(&make_routine("rs-manual-log-id", title)).unwrap();

        append_manual_trigger_log(&slug, 100);
        append_manual_trigger_log(&slug, 200);
        append_manual_trigger_log(&slug, 300);

        let log_path = crate::paths::routine_manual_log_path(&slug);
        let text = std::fs::read_to_string(&log_path).unwrap();
        assert_eq!(text, "100\n200\n300\n");
        // load reads the last (most recent) line.
        assert_eq!(
            load_routine_from_dir(&slug).unwrap().last_manual_trigger_at,
            Some(300)
        );
    });
}

#[test]
fn append_manual_trigger_log_warns_on_write_failure() {
    // Pointing the log path at a directory (so open fails) exercises the warn branch and
    // does not panic.
    let dir = scratch_dir("manual-log-fail");
    std::fs::create_dir_all(&dir).unwrap();
    // Create a directory where manual.log would be written, so the open call fails.
    let slug_dir = dir.join("rs-manual-log-fail-routine");
    std::fs::create_dir_all(&slug_dir).unwrap();
    let blocker = slug_dir.join("manual.log");
    std::fs::create_dir_all(&blocker).unwrap();

    // Override home so routine_manual_log_path resolves into our scratch dir.
    let previous = std::env::var_os("MOADIM_HOME_OVERRIDE");
    // SAFETY: single-threaded test execution.
    unsafe {
        std::env::set_var("MOADIM_HOME_OVERRIDE", &dir);
    }
    // Should not panic; just logs a warning.
    append_manual_trigger_log("rs-manual-log-fail-routine", 42);
    // SAFETY: single-threaded test execution.
    unsafe {
        match previous {
            Some(value) => std::env::set_var("MOADIM_HOME_OVERRIDE", value),
            None => std::env::remove_var("MOADIM_HOME_OVERRIDE"),
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn append_scheduled_trigger_log_warns_on_write_failure() {
    // A scheduler must still attempt the launcher when its best-effort evidence write fails.
    let dir = scratch_dir("scheduled-log-fail");
    let slug = "rs-scheduled-log-fail-routine";
    let blocker = dir.join(slug).join("scheduled.log");
    std::fs::create_dir_all(&blocker).unwrap();
    let previous = std::env::var_os("MOADIM_HOME_OVERRIDE");
    // SAFETY: single-threaded test execution.
    unsafe { std::env::set_var("MOADIM_HOME_OVERRIDE", &dir) };
    append_scheduled_trigger_log(slug, 42);
    // SAFETY: restore the process-wide test seam.
    unsafe {
        match previous {
            Some(value) => std::env::set_var("MOADIM_HOME_OVERRIDE", value),
            None => std::env::remove_var("MOADIM_HOME_OVERRIDE"),
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn append_skip_log_creates_and_appends() {
    // Each call appends one `{ts}\t{reason}` line; the log grows across calls (#1145).
    with_override_home(|_home| {
        let title = "Rs Skip Log Append Routine";
        let slug = slugify(title);
        write_routine(&make_routine("rs-skip-log-id", title)).unwrap();

        append_skip_log(&slug, 100, "overlap guard");
        append_skip_log(&slug, 200, "concurrency cap");

        let log_path = crate::paths::routine_skip_log_path(&slug);
        let text = std::fs::read_to_string(&log_path).unwrap();
        assert_eq!(text, "100\toverlap guard\n200\tconcurrency cap\n");
    });
}

#[test]
fn append_skip_log_warns_on_write_failure() {
    // Pointing the log path at a directory (so open fails) exercises the warn branch and
    // does not panic, mirroring `append_manual_trigger_log_warns_on_write_failure`.
    let dir = scratch_dir("skip-log-fail");
    std::fs::create_dir_all(&dir).unwrap();
    let slug_dir = dir.join("rs-skip-log-fail-routine");
    std::fs::create_dir_all(&slug_dir).unwrap();
    let blocker = slug_dir.join("skip.log");
    std::fs::create_dir_all(&blocker).unwrap();

    let previous = std::env::var_os("MOADIM_HOME_OVERRIDE");
    // SAFETY: single-threaded test execution.
    unsafe {
        std::env::set_var("MOADIM_HOME_OVERRIDE", &dir);
    }
    // Should not panic; just logs a warning.
    append_skip_log("rs-skip-log-fail-routine", 42, "agent load failure");
    // SAFETY: single-threaded test execution.
    unsafe {
        match previous {
            Some(value) => std::env::set_var("MOADIM_HOME_OVERRIDE", value),
            None => std::env::remove_var("MOADIM_HOME_OVERRIDE"),
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
}
