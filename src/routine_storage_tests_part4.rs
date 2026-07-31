
#[test]
fn load_routine_from_dir_applies_defaults_for_absent_optional_fields() {
    // A minimal routine.toml that omits prompt, enabled, timestamps, and id exercises the
    // default-fallback arms in load_routine_from_dir: prompt -> "", enabled -> true,
    // created_at/updated_at -> 0, and id -> dir_name (legacy fallback).
    with_override_home(|_home| {
        let slug = "rs-defaults-routine";
        let dir = crate::paths::routine_dir(slug);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            crate::paths::routine_toml_path(slug),
            "title = \"Rs Defaults Routine\"\nagent = \"claude\"\n",
        )
        .unwrap();
        std::fs::write(crate::paths::routine_cron_path(slug), "@daily\n").unwrap();

        let loaded = load_routine_from_dir(slug).unwrap();
        assert_eq!(loaded.id, slug, "absent id falls back to the dir name");
        assert_eq!(loaded.prompt, "", "absent prompt defaults to empty");
        assert!(loaded.enabled, "absent enabled defaults to true");
        assert_eq!(loaded.created_at, 0);
        assert_eq!(loaded.updated_at, 0);
        assert!(loaded.repositories.is_empty());
    });
}

#[test]
fn load_routine_from_dir_missing_returns_none() {
    with_override_home(|_home| {
        assert!(load_routine_from_dir("rs-does-not-exist-zzz").is_none());
    });
}

#[test]
fn load_routine_falls_back_to_legacy_last_triggered_in_routine_toml() {
    // A routine written by an older daemon stored `last_triggered_at` inside `routine.toml` and
    // has no sidecar. Load still surfaces the timestamp via the legacy-field fallback.
    with_override_home(|_home| {
        let slug = "rs-legacy-trigger-routine";
        let dir = crate::paths::routine_dir(slug);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            crate::paths::routine_toml_path(slug),
            "title = \"Rs Legacy Trigger\"\nagent = \"claude\"\nlast_triggered_at = 777\n",
        )
        .unwrap();
        std::fs::write(crate::paths::routine_cron_path(slug), "@daily\n").unwrap();
        // No sidecar exists yet.
        assert!(!crate::paths::routine_state_path(slug).exists());

        assert_eq!(
            load_routine_from_dir(slug).unwrap().last_manual_trigger_at,
            Some(777)
        );
    });
}

#[test]
fn load_routine_ignores_legacy_toml_schedule_when_cron_sidecar_exists() {
    // schedule.cron is the authoritative schedule source; a diverging legacy routine.toml schedule
    // is ignored.
    with_override_home(|_home| {
        let slug = "rs-toml-schedule-wins-routine";
        let dir = crate::paths::routine_dir(slug);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            crate::paths::routine_toml_path(slug),
            "schedule = \"@hourly\"\ntitle = \"Rs Toml Schedule Wins\"\nagent = \"claude\"\n",
        )
        .unwrap();
        std::fs::write(crate::paths::routine_cron_path(slug), "@weekly\n").unwrap();

        assert_eq!(load_routine_from_dir(slug).unwrap().schedule, "@weekly");
    });
}

// Gitignore-reconciliation tests live in `routine_storage_gitignore_tests.rs`, `[env]`
// table / `routine.local.toml` sidecar tests live in `routine_storage_env_tests.rs`, and
// `repersist_routines` tests live in `routine_storage_repersist_tests.rs` (all split out to keep
// this file under the line cap).

#[allow(
    clippy::missing_docs_in_private_items,
    reason = "split-out module keeps the file under the linecheck limit"
)]
#[path = "routine_storage_tests_part2.rs"]
mod routine_storage_tests_part2;

#[allow(
    clippy::missing_docs_in_private_items,
    reason = "split-out module keeps the file under the linecheck limit"
)]
#[path = "routine_storage_tests_part3.rs"]
mod routine_storage_tests_part3;
