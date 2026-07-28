#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use crate::routine_storage::routine_storage_migrations::seed_schedule_cron_from_legacy_toml;

#[test]
fn seed_schedule_cron_from_legacy_toml_writes_missing_sidecar() {
    let base =
        std::env::temp_dir().join(format!("moadim-rs-seed-schedule-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&base).unwrap();
    std::fs::write(
        base.join("routine.toml"),
        r#"title = "Legacy"
agent = "claude"
schedule = "@weekly"
"#,
    )
    .unwrap();

    seed_schedule_cron_from_legacy_toml(&base);

    assert_eq!(
        std::fs::read_to_string(base.join("schedule.cron")).unwrap(),
        "@weekly\n"
    );
    let _ = std::fs::remove_dir_all(base);
}

#[test]
fn seed_schedule_cron_from_legacy_toml_preserves_existing_sidecar() {
    let base = std::env::temp_dir().join(format!(
        "moadim-rs-seed-schedule-existing-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&base).unwrap();
    std::fs::write(base.join("schedule.cron"), "@hourly\n").unwrap();
    std::fs::write(
        base.join("routine.toml"),
        r#"title = "Legacy"
agent = "claude"
schedule = "@weekly"
"#,
    )
    .unwrap();

    seed_schedule_cron_from_legacy_toml(&base);

    assert_eq!(
        std::fs::read_to_string(base.join("schedule.cron")).unwrap(),
        "@hourly\n"
    );
    let _ = std::fs::remove_dir_all(base);
}

#[test]
fn seed_schedule_cron_from_legacy_toml_ignores_missing_legacy_schedule() {
    let base = std::env::temp_dir().join(format!(
        "moadim-rs-seed-schedule-missing-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&base).unwrap();
    std::fs::write(
        base.join("routine.toml"),
        r#"title = "Legacy"
agent = "claude"
"#,
    )
    .unwrap();

    seed_schedule_cron_from_legacy_toml(&base);

    assert!(!base.join("schedule.cron").exists());
    let _ = std::fs::remove_dir_all(base);
}

#[test]
#[cfg(unix)]
fn seed_schedule_cron_from_legacy_toml_logs_write_failure() {
    use std::os::unix::fs::PermissionsExt;

    let base = std::env::temp_dir().join(format!(
        "moadim-rs-seed-schedule-write-fail-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&base).unwrap();
    std::fs::write(
        base.join("routine.toml"),
        r#"title = "Legacy"
agent = "claude"
schedule = "@weekly"
"#,
    )
    .unwrap();
    let original_perms = std::fs::metadata(&base).unwrap().permissions();
    std::fs::set_permissions(&base, std::fs::Permissions::from_mode(0o555)).unwrap();

    seed_schedule_cron_from_legacy_toml(&base);

    std::fs::set_permissions(&base, original_perms).unwrap();
    assert!(!base.join("schedule.cron").exists());
    let _ = std::fs::remove_dir_all(base);
}
