#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and cases do not need doc comments"
)]

use super::*;
use crate::routines::{slugify, Routine};

fn with_override_home(body: impl FnOnce()) {
    let home = std::env::temp_dir().join(format!("moadim-rs-tags-home-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&home).unwrap();
    let previous = std::env::var_os("MOADIM_HOME_OVERRIDE");
    // SAFETY: the cargo config runs tests single-threaded (RUST_TEST_THREADS=1).
    unsafe {
        std::env::set_var("MOADIM_HOME_OVERRIDE", &home);
    }
    body();
    // SAFETY: single-threaded test execution.
    unsafe {
        match previous {
            Some(value) => std::env::set_var("MOADIM_HOME_OVERRIDE", value),
            None => std::env::remove_var("MOADIM_HOME_OVERRIDE"),
        }
    }
    let _ = std::fs::remove_dir_all(&home);
}

fn make_routine(id: &str, title: &str, tags: &[&str]) -> Routine {
    Routine {
        model: None,
        id: id.to_string(),
        schedule: "@daily".to_string(),
        schedules: vec![],
        title: title.to_string(),
        agent: "claude".to_string(),
        prompt: "task".to_string(),
        goal: None,
        repositories: vec![],
        machines: vec![crate::machine::current_machine()],
        enabled: true,
        disabled_reason: None,
        source: "managed".to_string(),
        created_at: 5,
        updated_at: 6,
        last_manual_trigger_at: None,
        last_scheduled_trigger_at: None,
        snoozed_until: None,
        skip_runs: None,
        power_saving: false,
        power_saving_exempt: false,
        tags: tags.iter().map(ToString::to_string).collect(),
        ttl_secs: None,
        max_runtime_secs: None,
        env: std::collections::HashMap::new(),
        auto_disabled_reason: None,
        consecutive_failures: 0,
        failure_threshold: None,
        notifications: Default::default(),
        timezone: None,
    }
}

#[test]
fn tags_round_trip_through_tags_csv_sidecar() {
    with_override_home(|| {
        let title = "Tags Csv Round Trip";
        let slug = slugify(title);
        let routine = make_routine("tags-csv-round-trip", title, &["nightly", "a,b"]);
        write_routine(&routine).unwrap();

        assert_eq!(
            std::fs::read_to_string(crate::paths::routine_tags_path(&slug)).unwrap(),
            "nightly,\"a,b\"\n"
        );
        let toml_text = std::fs::read_to_string(crate::paths::routine_toml_path(&slug)).unwrap();
        assert!(
            !toml_text.contains("tags ="),
            "routine.toml must not carry tags: {toml_text}"
        );
        assert_eq!(
            load_routine_from_dir(&slug).unwrap().tags,
            vec!["nightly".to_string(), "a,b".to_string()]
        );
        remove_routine_dir(&slug).unwrap();
    });
}

#[test]
fn single_tag_is_written_to_sidecar_and_absent_from_routine_toml() {
    with_override_home(|| {
        let title = "Tags Csv Single";
        let slug = slugify(title);
        write_routine(&make_routine("tags-csv-single", title, &["a"])).unwrap();

        let toml_text = std::fs::read_to_string(crate::paths::routine_toml_path(&slug)).unwrap();
        assert!(
            !toml_text.lines().any(|line| line.trim_start().starts_with("tags")),
            "routine.toml must have no tags key: {toml_text}"
        );
        assert_eq!(
            load_routine_from_dir(&slug).unwrap().tags,
            vec!["a".to_string()]
        );
        remove_routine_dir(&slug).unwrap();
    });
}

#[test]
fn empty_tags_write_empty_sidecar_and_ignore_legacy_toml_key() {
    with_override_home(|| {
        let title = "Tags Csv Empty";
        let slug = slugify(title);
        write_routine(&make_routine("tags-csv-empty", title, &[])).unwrap();

        let tags_path = crate::paths::routine_tags_path(&slug);
        assert_eq!(std::fs::metadata(&tags_path).unwrap().len(), 0);

        // A legacy key left in routine.toml must not win over the present, empty tags.csv.
        let toml_path = crate::paths::routine_toml_path(&slug);
        let text = std::fs::read_to_string(&toml_path).unwrap();
        std::fs::write(&toml_path, format!("tags = [\"old\"]\n{text}")).unwrap();
        assert_eq!(
            load_routine_from_dir(&slug).unwrap().tags,
            Vec::<String>::new()
        );
        remove_routine_dir(&slug).unwrap();
    });
}

#[test]
fn legacy_routine_toml_tags_load_then_migration_seeds_tags_csv() {
    with_override_home(|| {
        let title = "Tags Csv Legacy";
        let slug = slugify(title);
        let dir = crate::paths::routine_dir(&slug);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("routine.toml"),
            "id = \"tags-csv-legacy\"\ntitle = \"Tags Csv Legacy\"\nagent = \"claude\"\ntags = [\"x\"]\n",
        )
        .unwrap();
        std::fs::write(dir.join("schedule.cron"), "@daily\n").unwrap();

        assert_eq!(
            load_routine_from_dir(&slug).unwrap().tags,
            vec!["x".to_string()]
        );

        migrate_routine_dirs_from_dir(&crate::paths::routines_dir());
        assert_eq!(
            std::fs::read_to_string(crate::paths::routine_tags_path(&slug)).unwrap(),
            "x\n"
        );
        remove_routine_dir(&slug).unwrap();
    });
}

#[test]
fn malformed_tags_csv_blocks_persist_and_keeps_routine_toml() {
    with_override_home(|| {
        let title = "Tags Csv Malformed";
        let slug = slugify(title);
        write_routine(&make_routine("tags-csv-malformed", title, &["a"])).unwrap();
        let csv_path = crate::paths::routine_tags_path(&slug);
        let toml_path = crate::paths::routine_toml_path(&slug);
        std::fs::write(&csv_path, b"\xff\n").unwrap();
        let toml_before = std::fs::read(&toml_path).unwrap();

        assert!(write_routine(&make_routine("tags-csv-malformed", title, &["b"])).is_err());

        assert_eq!(std::fs::read(&csv_path).unwrap(), b"\xff\n");
        assert_eq!(std::fs::read(&toml_path).unwrap(), toml_before);
        remove_routine_dir(&slug).unwrap();
    });
}
