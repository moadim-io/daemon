#![allow(clippy::missing_docs_in_private_items)]

use super::*;

use crate::routines::new_store;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Point `MOADIM_HOME_OVERRIDE` at a fresh, empty temp home for the duration of a test, removing the
/// env var and the temp dir on drop. This keeps `svc_create`/`svc_update`/`write_routine` and the
/// other disk-touching paths off the developer's real `~/.moadim`, so a panicking assertion can never
/// leak test routines into the real home. Tests in this crate run single-threaded
/// (`RUST_TEST_THREADS=1`), so the global env mutation is safe.
struct TempHome(std::path::PathBuf);

impl TempHome {
    fn set() -> Self {
        let dir = std::env::temp_dir().join(format!("moadim-svctest-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("create temp home");
        // SAFETY: single-threaded test execution.
        unsafe {
            std::env::set_var("MOADIM_HOME_OVERRIDE", &dir);
        }
        Self(dir)
    }
}

impl Drop for TempHome {
    fn drop(&mut self) {
        // SAFETY: single-threaded test execution.
        unsafe {
            std::env::remove_var("MOADIM_HOME_OVERRIDE");
        }
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Build a routine with overridable identity, title, timestamps, and repository URL.
fn make_routine(id: &str, title: &str, created_at: u64, updated_at: u64) -> Routine {
    Routine {
        model: None,
        id: id.to_string(),
        schedule: "@daily".to_string(),
        title: title.to_string(),
        agent: "claude".to_string(),
        prompt: "do the thing".to_string(),
        goal: None,
        repositories: vec![],
        machines: vec![crate::machine::current_machine()],
        enabled: true,
        source: "managed".to_string(),
        created_at,
        updated_at,
        last_manual_trigger_at: None,
        last_scheduled_trigger_at: None,
        snoozed_until: None,
        skip_runs: None,
        power_saving: false,
        tags: vec![],
        ttl_secs: None,
        max_runtime_secs: None,
    }
}

/// Wrap a list of routines into a populated [`RoutineStore`].
fn store_with(routines: Vec<Routine>) -> RoutineStore {
    let mut map = HashMap::new();
    for routine in routines {
        map.insert(routine.id.clone(), routine);
    }
    Arc::new(Mutex::new(map))
}

#[test]
fn svc_list_sorts_by_updated_at() {
    let _home = TempHome::set();
    // Covers the `RoutineSort::Updated` arm: sort by `updated_at` ascending.
    let store = store_with(vec![
        make_routine("late", "Zeta", 100, 300),
        make_routine("early", "Alpha", 100, 100),
        make_routine("mid", "Mid", 100, 200),
    ]);
    let query = RoutineListQuery {
        sort: RoutineSort::Updated,
        ..Default::default()
    };
    let list = svc_list(&store, &query);
    assert_eq!(list[0].routine.id, "early");
    assert_eq!(list[1].routine.id, "mid");
    assert_eq!(list[2].routine.id, "late");
}

#[test]
fn svc_list_sorts_by_title_case_insensitively() {
    let _home = TempHome::set();
    // Covers the `RoutineSort::Title` arm: sort by lowercased title.
    let store = store_with(vec![
        make_routine("banana", "banana", 0, 0),
        make_routine("apple", "Apple", 0, 0),
        make_routine("cherry", "CHERRY", 0, 0),
    ]);
    let query = RoutineListQuery {
        sort: RoutineSort::Title,
        ..Default::default()
    };
    let list = svc_list(&store, &query);
    assert_eq!(list[0].routine.id, "apple");
    assert_eq!(list[1].routine.id, "banana");
    assert_eq!(list[2].routine.id, "cherry");
}

#[test]
fn svc_list_omits_prompt_by_default() {
    let _home = TempHome::set();
    // Default query leaves the prompt blank, and `skip_serializing_if` drops the field entirely.
    let store = store_with(vec![make_routine("a", "Alpha", 0, 0)]);
    let list = svc_list(&store, &RoutineListQuery::default());
    assert_eq!(list.len(), 1);
    assert!(list[0].routine.prompt.is_empty());
    let json = serde_json::to_value(&list[0]).unwrap();
    assert!(
        json.get("prompt").is_none(),
        "prompt should be absent from the serialized listing, got {json}"
    );
}

#[test]
fn svc_list_includes_prompt_when_requested() {
    let _home = TempHome::set();
    let store = store_with(vec![make_routine("a", "Alpha", 0, 0)]);
    let query = RoutineListQuery {
        include_prompts: Some(true),
        ..Default::default()
    };
    let list = svc_list(&store, &query);
    assert_eq!(list[0].routine.prompt, "do the thing");
    let json = serde_json::to_value(&list[0]).unwrap();
    assert_eq!(
        json.get("prompt").and_then(|value| value.as_str()),
        Some("do the thing")
    );
}

/// Build a minimal valid create request; callers tweak the field under test.
fn valid_create_request() -> CreateRoutineRequest {
    CreateRoutineRequest {
        model: None,
        schedule: "@daily".into(),
        title: "Valid Title".into(),
        agent: "claude".into(),
        prompt: "do the thing".into(),
        goal: None,
        repositories: vec![],
        machines: vec![crate::machine::current_machine()],
        enabled: true,
        ttl_secs: None,
        max_runtime_secs: None,
        tags: vec![],
    }
}

/// Build a no-op update request (every field `None`); callers set one field.
fn empty_update_request() -> UpdateRoutineRequest {
    UpdateRoutineRequest {
        model: None,
        schedule: None,
        title: None,
        agent: None,
        prompt: None,
        goal: None,
        repositories: None,
        machines: None,
        enabled: None,
        ttl_secs: None,
        max_runtime_secs: None,
        tags: None,
    }
}

#[test]
fn svc_create_rejects_blank_title() {
    let _home = TempHome::set();
    // Covers the `reject_blank("title", ..)` error arm in `svc_create`: a
    // whitespace-only title is refused before any slug/disk work (#226).
    let store = new_store();
    let result = svc_create(
        &store,
        CreateRoutineRequest {
            model: None,
            title: "   ".into(),
            ..valid_create_request()
        },
    );
    assert!(matches!(result, Err(AppError::BadRequest(_))));
}

#[test]
fn svc_create_rejects_blank_prompt() {
    let _home = TempHome::set();
    // Covers the `reject_blank("prompt", ..)` error arm in `svc_create`: an empty
    // prompt would make the routine fire forever with no task (#224).
    let store = new_store();
    let result = svc_create(
        &store,
        CreateRoutineRequest {
            model: None,
            prompt: String::new(),
            goal: None,
            ..valid_create_request()
        },
    );
    assert!(matches!(result, Err(AppError::BadRequest(_))));
}

#[test]
fn svc_create_rejects_zero_ttl_secs() {
    let _home = TempHome::set();
    // Covers the `reject_zero_secs("ttl_secs", ..)` error arm in `svc_create`:
    // a zero TTL reaps finished-run logs instantly (#233).
    let store = new_store();
    let result = svc_create(
        &store,
        CreateRoutineRequest {
            model: None,
            ttl_secs: Some(0),
            ..valid_create_request()
        },
    );
    assert!(matches!(result, Err(AppError::BadRequest(_))));
}

#[test]
fn svc_create_rejects_zero_max_runtime_secs() {
    let _home = TempHome::set();
    // Covers the `reject_zero_secs("max_runtime_secs", ..)` error arm in
    // `svc_create`: a zero cap self-kills the run immediately (#233).
    let store = new_store();
    let result = svc_create(
        &store,
        CreateRoutineRequest {
            model: None,
            max_runtime_secs: Some(0),
            tags: vec![],
            ..valid_create_request()
        },
    );
    assert!(matches!(result, Err(AppError::BadRequest(_))));
}

#[test]
fn svc_create_persists_machines() {
    let _home = TempHome::set();
    // Covers the `machines: req.machines` assignment in `svc_create`.
    let store = new_store();
    let resp = svc_create(
        &store,
        CreateRoutineRequest {
            model: None,
            machines: vec!["alpha".into(), "beta".into()],
            ..valid_create_request()
        },
    )
    .expect("create");
    assert_eq!(resp.routine.machines, vec!["alpha", "beta"]);
}

#[test]
fn svc_update_sets_machines() {
    let _home = TempHome::set();
    // Covers the `if let Some(machines) = req.machines` branch in `svc_update`.
    let store = store_with(vec![make_routine("upd-machines", "Keep", 1, 1)]);
    let resp = svc_update(
        &store,
        "upd-machines",
        UpdateRoutineRequest {
            model: None,
            machines: Some(vec!["server".into()]),
            ..empty_update_request()
        },
    )
    .expect("update");
    assert_eq!(resp.routine.machines, vec!["server"]);
}

#[test]
fn svc_update_rejects_blank_title() {
    let _home = TempHome::set();
    // Covers the `reject_blank("title", ..)` error arm in `svc_update`.
    let store = store_with(vec![make_routine("upd-blank-title", "Keep", 1, 1)]);
    let result = svc_update(
        &store,
        "upd-blank-title",
        UpdateRoutineRequest {
            model: None,
            title: Some("  ".into()),
            ..empty_update_request()
        },
    );
    assert!(matches!(result, Err(AppError::BadRequest(_))));
}

#[test]
fn svc_update_rejects_blank_prompt() {
    let _home = TempHome::set();
    // Covers the `reject_blank("prompt", ..)` error arm in `svc_update`.
    let store = store_with(vec![make_routine("upd-blank-prompt", "Keep", 1, 1)]);
    let result = svc_update(
        &store,
        "upd-blank-prompt",
        UpdateRoutineRequest {
            model: None,
            prompt: Some("\t\n".into()),
            goal: None,
            ..empty_update_request()
        },
    );
    assert!(matches!(result, Err(AppError::BadRequest(_))));
}

#[test]
fn svc_update_rejects_goal_over_max_lines() {
    let _home = TempHome::set();
    // Covers the `validate_goal(Some(goal))?` error arm in `svc_update` (goal validation
    // runs before the routine-existence check, so a non-existent id is fine here).
    let store = store_with(vec![]);
    let result = svc_update(
        &store,
        "missing",
        UpdateRoutineRequest {
            model: None,
            goal: Some("l1\nl2\nl3\nl4\nl5\nl6".into()),
            ..empty_update_request()
        },
    );
    assert!(matches!(result, Err(AppError::BadRequest(_))));
}

#[test]
fn svc_update_rejects_zero_durations() {
    let _home = TempHome::set();
    // Covers both `reject_zero_secs` error arms on the update path.
    let store = store_with(vec![make_routine("upd-zero-secs", "Keep", 1, 1)]);
    let ttl = svc_update(
        &store,
        "upd-zero-secs",
        UpdateRoutineRequest {
            model: None,
            ttl_secs: Some(0),
            ..empty_update_request()
        },
    );
    assert!(matches!(ttl, Err(AppError::BadRequest(_))));
    let max_runtime = svc_update(
        &store,
        "upd-zero-secs",
        UpdateRoutineRequest {
            model: None,
            max_runtime_secs: Some(0),
            tags: None,
            ..empty_update_request()
        },
    );
    assert!(matches!(max_runtime, Err(AppError::BadRequest(_))));
}

#[test]
fn svc_create_rejects_ttl_above_cron_ceiling() {
    let _home = TempHome::set();
    // A `*/5 * * * *` routine has a ttl ceiling of min(3600, 300) = 300s. An explicit 1800 would be
    // silently clamped to 300, so it is rejected with `BadRequest` up front (#468).
    let store = new_store();
    let result = svc_create(
        &store,
        CreateRoutineRequest {
            model: None,
            schedule: "*/5 * * * *".into(),
            ttl_secs: Some(1800),
            ..valid_create_request()
        },
    );
    assert!(matches!(result, Err(AppError::BadRequest(_))));
}

#[test]
fn svc_create_rejects_max_runtime_above_cron_ceiling() {
    let _home = TempHome::set();
    // Mirror of the ttl ceiling for the watchdog bound (#468).
    let store = new_store();
    let result = svc_create(
        &store,
        CreateRoutineRequest {
            model: None,
            schedule: "*/5 * * * *".into(),
            max_runtime_secs: Some(1800),
            ..valid_create_request()
        },
    );
    assert!(matches!(result, Err(AppError::BadRequest(_))));
}

#[test]
fn svc_create_accepts_secs_at_cron_ceiling() {
    let _home = TempHome::set();
    // A value equal to the cron-derived ceiling (`*/5` -> 300s) is in force, not clamped, so it
    // passes `reject_over_ceiling` (covering the `secs <= ceiling` arm for both fields). A
    // duplicate-slug routine pre-seeded in the store makes the create fail *after* that check with a
    // `Conflict`, so the assertion proves the ceiling check did not reject — without performing any
    // crontab/disk mutation.
    let store = store_with(vec![make_routine(
        "at-ceiling-dupe",
        "At Ceiling ZZZ",
        1,
        1,
    )]);
    let result = svc_create(
        &store,
        CreateRoutineRequest {
            model: None,
            schedule: "*/5 * * * *".into(),
            // Same slug as the pre-seeded routine.
            title: "  at   ceiling ZZZ ".into(),
            ttl_secs: Some(300),
            max_runtime_secs: Some(300),
            ..valid_create_request()
        },
    );
    assert!(matches!(result, Err(AppError::Conflict(_))));
}

#[test]
fn svc_update_rejects_ttl_above_current_schedule_ceiling() {
    let _home = TempHome::set();
    // No schedule supplied: the ceiling derives from the routine's *current* `*/5` schedule, so a
    // 1800s ttl exceeds the 300s ceiling and is rejected without mutating the store (#468).
    let store = store_with(vec![Routine {
        schedule: "*/5 * * * *".to_string(),
        ..make_routine("upd-ttl-ceiling", "Keep Ceiling", 1, 1)
    }]);
    let result = svc_update(
        &store,
        "upd-ttl-ceiling",
        UpdateRoutineRequest {
            model: None,
            ttl_secs: Some(1800),
            ..empty_update_request()
        },
    );
    assert!(matches!(result, Err(AppError::BadRequest(_))));
    // The store value is untouched by the rejected update.
    assert_eq!(
        store
            .lock()
            .unwrap()
            .get("upd-ttl-ceiling")
            .unwrap()
            .ttl_secs,
        None
    );
}

#[test]
fn svc_update_rejects_secs_above_new_schedule_ceiling() {
    let _home = TempHome::set();
    // A supplied schedule is the *effective* schedule for the ceiling: tightening a `@daily` routine
    // to `*/5` while setting max_runtime 1800 exceeds the new 300s ceiling and is rejected (#468).
    let store = store_with(vec![make_routine("upd-new-sched", "Keep New Sched", 1, 1)]);
    let result = svc_update(
        &store,
        "upd-new-sched",
        UpdateRoutineRequest {
            model: None,
            schedule: Some("*/5 * * * *".into()),
            max_runtime_secs: Some(1800),
            ..empty_update_request()
        },
    );
    assert!(matches!(result, Err(AppError::BadRequest(_))));
}
