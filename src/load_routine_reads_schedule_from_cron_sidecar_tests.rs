#![allow(
    clippy::wildcard_imports,
    clippy::too_many_arguments,
    clippy::needless_pass_by_value,
    dead_code,
    reason = "split-out module preserves existing code while keeping files under the linecheck limit"
)]
use super::*;

#[test]
fn load_routine_reads_schedule_from_cron_sidecar() {
    // The schedule lives in schedule.cron. Comment lines are skipped when reading the sidecar.
    with_override_home(|_home| {
        let slug = "rs-cron-fallback-routine";
        let dir = crate::paths::routine_dir(slug);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            crate::paths::routine_toml_path(slug),
            "title = \"Rs Cron Fallback\"\nagent = \"claude\"\n",
        )
        .unwrap();
        std::fs::write(
            crate::paths::routine_cron_path(slug),
            "# some comment\n\n@hourly\n",
        )
        .unwrap();

        assert_eq!(load_routine_from_dir(slug).unwrap().schedule, "@hourly");
    });
}

#[test]
fn load_routine_blank_schedule_cron_with_no_legacy_schedule_returns_none() {
    // A `schedule.cron` with no cron line — only blanks and comments (e.g. truncated by a crash
    // mid-write) — parses to `None` rather than an empty string, and with no `schedule` field in
    // `routine.toml` either, the whole load short-circuits to `None` instead of a routine with a
    // blank schedule.
    with_override_home(|_home| {
        let slug = "rs-blank-schedule-cron-routine";
        let dir = crate::paths::routine_dir(slug);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            crate::paths::routine_toml_path(slug),
            "title = \"Rs Blank Schedule Cron\"\nagent = \"claude\"\n",
        )
        .unwrap();
        std::fs::write(
            crate::paths::routine_cron_path(slug),
            "# not functional yet\n\n",
        )
        .unwrap();

        assert!(load_routine_from_dir(slug).is_none());
    });
}

#[test]
fn load_routine_ignores_unparsable_sidecar() {
    // A malformed `state.local.toml` parses to `None` (rather than crashing the load), and with no
    // legacy field in `routine.toml` the routine loads with no trigger timestamp.
    with_override_home(|_home| {
        let slug = "rs-bad-sidecar-routine";
        let dir = crate::paths::routine_dir(slug);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            crate::paths::routine_toml_path(slug),
            "title = \"Rs Bad Sidecar\"\nagent = \"claude\"\n",
        )
        .unwrap();
        std::fs::write(crate::paths::routine_cron_path(slug), "@daily\n").unwrap();
        std::fs::write(crate::paths::routine_state_path(slug), "= not valid toml =").unwrap();

        assert_eq!(
            load_routine_from_dir(slug).unwrap().last_manual_trigger_at,
            None
        );
    });
}

#[test]
fn load_routine_reads_scheduled_trigger_from_log() {
    // `last_scheduled_trigger_at` is read from the last line of `scheduled.log`, written by the
    // cron shell command at each fire, independently of the manual-trigger log.
    with_override_home(|_home| {
        let title = "Rs Scheduled Sidecar Routine";
        let slug = slugify(title);
        write_routine(&make_routine("rs-scheduled-id", title)).unwrap();
        // Simulate two cron fires appended to scheduled.log.
        std::fs::write(
            crate::paths::routine_scheduled_log_path(&slug),
            "1000\n4242\n",
        )
        .unwrap();

        let loaded = load_routine_from_dir(&slug).unwrap();
        // The last line (4242) wins.
        assert_eq!(loaded.last_scheduled_trigger_at, Some(4242));
        // The scheduled timestamp is distinct from the (unset) manual one.
        assert_eq!(loaded.last_manual_trigger_at, None);
    });
}

#[test]
fn load_routine_ignores_unparsable_scheduled_log() {
    // A `scheduled.log` with no parsable timestamp lines yields `None` rather than crashing.
    with_override_home(|_home| {
        let slug = "rs-bad-scheduled-sidecar-routine";
        let dir = crate::paths::routine_dir(slug);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            crate::paths::routine_toml_path(slug),
            "title = \"Rs Bad Scheduled Sidecar\"\nagent = \"claude\"\n",
        )
        .unwrap();
        std::fs::write(crate::paths::routine_cron_path(slug), "@daily\n").unwrap();
        std::fs::write(
            crate::paths::routine_scheduled_log_path(slug),
            "not a timestamp\n",
        )
        .unwrap();

        assert_eq!(
            load_routine_from_dir(slug)
                .unwrap()
                .last_scheduled_trigger_at,
            None
        );
    });
}

#[test]
fn write_routine_preserves_scheduler_written_scheduled_log() {
    // The daemon never writes `scheduled.log`, so re-persisting a routine must leave the
    // cron-appended log untouched — the same invariant that motivated the separate-file design.
    with_override_home(|_home| {
        let title = "Rs Preserve Scheduled Routine";
        let slug = slugify(title);
        let mut routine = make_routine("rs-preserve-scheduled-id", title);
        write_routine(&routine).unwrap();

        // Simulate a scheduled cron firing appending to scheduled.log.
        std::fs::write(crate::paths::routine_scheduled_log_path(&slug), "55\n").unwrap();

        // A subsequent daemon-side write (manual trigger recorded, routine updated, repersist, …).
        routine.last_manual_trigger_at = Some(7);
        write_routine(&routine).unwrap();
        crate::routine_storage::append_manual_trigger_log(&slug, 7);

        assert!(
            crate::paths::routine_scheduled_log_path(&slug).exists(),
            "daemon write must not remove the scheduler-owned log"
        );
        let loaded = load_routine_from_dir(&slug).unwrap();
        assert_eq!(loaded.last_scheduled_trigger_at, Some(55));
        assert_eq!(loaded.last_manual_trigger_at, Some(7));
    });
}

#[test]
fn torn_routine_toml_loads_as_none() {
    // A truncated/garbage routine.toml (e.g. left by a crash mid-write) must not panic or load a
    // half-baked routine; the loader returns None and the routine is simply absent.
    with_override_home(|_home| {
        let slug = "rs-torn-toml-routine";
        let dir = crate::paths::routine_dir(slug);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(crate::paths::routine_toml_path(slug), "id = \"x\"\nschedu").unwrap();
        assert!(load_routine_from_dir(slug).is_none());
    });
}
include!("load_store_includes_written_routine_tests.rs");
