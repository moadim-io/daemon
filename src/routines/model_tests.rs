#![allow(clippy::missing_docs_in_private_items)]

use super::*;

#[test]
fn describe_schedule_appends_timezone_when_present() {
    let desc = describe_schedule("@daily", Some("Asia/Jerusalem")).unwrap();
    assert!(
        desc.ends_with("(Asia/Jerusalem)"),
        "expected timezone suffix in {desc}"
    );
}

#[test]
fn describe_schedule_omits_timezone_when_none() {
    // The `None` arm returns the bare description with no parenthesized timezone.
    let desc = describe_schedule("@daily", None).unwrap();
    assert!(!desc.contains('('), "expected no timezone suffix in {desc}");
}

#[test]
fn describe_schedule_returns_none_for_unparseable() {
    assert!(describe_schedule("@reboot", Some("UTC")).is_none());
    assert!(describe_schedule("not a cron", None).is_none());
}

#[test]
fn from_routine_populates_derived_fields() {
    let routine = Routine {
        id: "rid".into(),
        schedule: "@daily".into(),
        title: "My Title".into(),
        agent: "claude".into(),
        prompt: "p".into(),
        repositories: vec![],
        machines: vec![crate::machine::current_machine()],
        enabled: true,
        source: "managed".into(),
        created_at: 0,
        updated_at: 0,
        last_manual_trigger_at: None,
        last_scheduled_trigger_at: None,
        tags: vec![],
        ttl_secs: None,
        max_runtime_secs: None,
    };
    let resp = RoutineResponse::from_routine(routine);
    assert!(resp.schedule_description.is_some());
    assert!(resp.file_path.contains("routine.toml"));
    assert_eq!(resp.flag_count, 0);
}

#[test]
fn from_routine_counts_open_flags() {
    let routine = Routine {
        id: "rid2".into(),
        schedule: "@daily".into(),
        title: "Flag Count Model Test ZZZ".into(),
        agent: "claude".into(),
        prompt: "p".into(),
        repositories: vec![],
        machines: vec![crate::machine::current_machine()],
        enabled: true,
        source: "managed".into(),
        created_at: 0,
        updated_at: 0,
        last_manual_trigger_at: None,
        last_scheduled_trigger_at: None,
        tags: vec![],
        ttl_secs: None,
        max_runtime_secs: None,
    };
    let slug = slugify(&routine.title);
    crate::routines::flags::create_flag(
        &slug,
        "bug",
        "d1",
        crate::routines::flags::FlagScope::General,
    )
    .unwrap();
    crate::routines::flags::create_flag(
        &slug,
        "gap",
        "d2",
        crate::routines::flags::FlagScope::Local,
    )
    .unwrap();

    let resp = RoutineResponse::from_routine(routine);
    assert_eq!(resp.flag_count, 2);

    crate::routine_storage::remove_routine_dir(&slug).unwrap();
}
