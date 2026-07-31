#![allow(
    clippy::wildcard_imports,
    clippy::too_many_arguments,
    clippy::needless_pass_by_value,
    dead_code,
    reason = "split-out module preserves existing code while keeping files under the linecheck limit"
)]
use super::*;

#[test]
fn svc_create_persists_machines() {
    let _home = TempHome::set();
    // Covers the `machines: req.machines` assignment in `svc_create`.
    let store = new_store();
    let resp = svc_create(
        &store,
        CreateRoutineRequest {
            model: None,
            goal: None,
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
            goal: None,
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
            goal: None,
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
            goal: None,
            prompt: Some("\t\n".into()),
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
            goal: None,
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
            goal: None,
            max_runtime_secs: Some(0),
            tags: None,
            ..empty_update_request()
        },
    );
    assert!(matches!(max_runtime, Err(AppError::BadRequest(_))));
}

#[test]
fn svc_create_rejects_agent_config_without_prompt_placeholder() {
    // A parseable, registered agent whose `args` carry no `{prompt}`/`{prompt_file}` placeholder
    // would launch with no task. It is rejected at create time with `BadRequest` (#322), not
    // silently fired forever.
    let agent_name = "svc-create-noprompt-agent-zzz";
    std::fs::create_dir_all(crate::paths::agents_dir()).unwrap();
    let cfg = crate::paths::agent_toml_path(agent_name);
    std::fs::write(&cfg, "command = \"echo\"\nargs = [\"{workbench}\"]\n").unwrap();

    let store = new_store();
    let result = svc_create(
        &store,
        CreateRoutineRequest {
            model: None,
            goal: None,
            title: "Svc Create NoPrompt ZZZ".into(),
            agent: agent_name.into(),
            prompt: "p".into(),
            ..valid_create_request()
        },
    );
    match result {
        Err(AppError::BadRequest(msg)) => {
            assert!(msg.contains("config:"), "{msg}");
            assert!(msg.contains("must include a prompt placeholder"), "{msg}");
        }
        other => panic!("expected BadRequest, got {other:?}"),
    }

    std::fs::remove_file(&cfg).unwrap();
}
include!("svc_create_maps_an_on_disk_slug_collision_to_conflict_tests.rs");
