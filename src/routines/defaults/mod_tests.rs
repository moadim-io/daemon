#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use super::*;
use crate::routines::available_agents;
use croner::Cron;

#[test]
fn ships_at_least_one_default() {
    assert!(!DEFAULT_ROUTINES.is_empty());
}

#[test]
fn first_default_updates_moadim_cargo_package() {
    let first = &DEFAULT_ROUTINES[0];
    assert_eq!(first.title, "Update moadim cargo package");
    assert!(first.prompt.contains("cargo install moadim --force"));
}

#[test]
fn second_default_is_the_1_percent() {
    let spec = &DEFAULT_ROUTINES[1];
    assert_eq!(spec.title, "The 1 Percent");
    assert!(spec.prompt.contains("list_routines"));
    assert!(spec.prompt.contains("update_routine"));
    assert!(spec.prompt.contains("NOT_REPO"));
}

#[test]
fn third_default_is_token_trim() {
    let spec = &DEFAULT_ROUTINES[2];
    assert_eq!(spec.title, "Token Trim");
    assert!(spec.prompt.contains("list_routines"));
    assert!(spec.prompt.contains("update_routine"));
    assert!(spec.prompt.contains("NOT_REPO"));
    assert!(spec.prompt.contains("token"));
}

#[test]
fn every_schedule_is_a_valid_cron() {
    for spec in DEFAULT_ROUTINES {
        let normalized = normalize_schedule(spec.schedule);
        assert!(
            normalized.parse::<Cron>().is_ok(),
            "schedule for {:?} is not a valid cron: {normalized:?}",
            spec.title
        );
    }
}

#[test]
fn every_agent_is_a_known_builtin() {
    let known = available_agents();
    for spec in DEFAULT_ROUTINES {
        assert!(
            known.iter().any(|agent| agent == spec.agent),
            "agent {:?} for routine {:?} is not a built-in agent",
            spec.agent,
            spec.title
        );
    }
}

#[test]
fn materialize_stamps_timestamps_and_marks_managed() {
    let spec = &DEFAULT_ROUTINES[0];
    let routine = materialize(spec, 1234);
    assert_eq!(routine.created_at, 1234);
    assert_eq!(routine.updated_at, 1234);
    assert_eq!(routine.source, "managed");
    assert!(routine.enabled);
    assert!(routine.last_manual_trigger_at.is_none());
    assert!(!routine.id.is_empty());
    // Schedule is normalized, not the raw spec string.
    assert_eq!(routine.schedule, normalize_schedule(spec.schedule));
}

#[test]
fn materialize_assigns_unique_ids() {
    let spec = &DEFAULT_ROUTINES[0];
    assert_ne!(materialize(spec, 0).id, materialize(spec, 0).id);
}

#[test]
fn reconcile_returns_none_when_up_to_date() {
    let spec = &DEFAULT_ROUTINES[0];
    let cur = materialize(spec, 100);
    assert!(reconcile(spec, &cur, 200).is_none());
}

#[test]
fn reconcile_preserves_disabled_toggle() {
    let spec = &DEFAULT_ROUTINES[0];
    // User turned the default off and an old prompt is on disk: it must be refreshed but stay off.
    let mut cur = materialize(spec, 100);
    cur.enabled = false;
    cur.prompt = "stale prompt".to_string();
    let updated = reconcile(spec, &cur, 200).expect("drifted routine should be rewritten");
    assert!(
        !updated.enabled,
        "must not re-enable a user-disabled default"
    );
    assert_eq!(updated.prompt, spec.prompt, "prompt should be refreshed");
}

#[test]
fn reconcile_preserves_power_saving() {
    let spec = &DEFAULT_ROUTINES[0];
    // Power saving is daemon/policy-owned, not spec-derived — a content drift refresh must not
    // clear it, the same way it must not touch `enabled`.
    let mut cur = materialize(spec, 100);
    cur.power_saving = true;
    cur.prompt = "stale prompt".to_string();
    let updated = reconcile(spec, &cur, 200).expect("drifted routine should be rewritten");
    assert!(
        updated.power_saving,
        "must not clear power-saving state on a content refresh"
    );
}

#[test]
fn reconcile_refreshes_content_but_keeps_identity() {
    let spec = &DEFAULT_ROUTINES[0];
    let mut cur = materialize(spec, 100);
    cur.schedule = "0 0 * * *".to_string();
    let updated = reconcile(spec, &cur, 200).expect("schedule drift should be rewritten");
    assert_eq!(updated.schedule, normalize_schedule(spec.schedule));
    // Identity and history are carried over; only updated_at advances.
    assert_eq!(updated.id, cur.id);
    assert_eq!(updated.created_at, cur.created_at);
    assert_eq!(updated.updated_at, 200);
}

#[test]
fn reconcile_keeps_enabled_default_enabled() {
    let spec = &DEFAULT_ROUTINES[0];
    let mut cur = materialize(spec, 100);
    cur.prompt = "stale".to_string();
    let updated = reconcile(spec, &cur, 200).expect("drift should be rewritten");
    assert!(updated.enabled);
}

#[test]
fn reconcile_treats_empty_machines_as_drift_and_seeds_current_machine() {
    // Legacy default routines seeded before machine-awareness were stored with an empty
    // `machines` list, leaving them permanently dormant. `reconcile` must detect this
    // as drift (even when all other daemon-owned fields are current) and seed the current
    // machine so the routine becomes active. (#723)
    let spec = &DEFAULT_ROUTINES[0];
    let mut cur = materialize(spec, 100);
    cur.machines = Vec::new(); // simulate pre-machine-awareness legacy state
    let updated = reconcile(spec, &cur, 200)
        .expect("empty machines list must be treated as drift and trigger a rewrite");
    assert!(
        !updated.machines.is_empty(),
        "reconcile must seed the current machine when cur.machines is empty"
    );
}
include!("reconcile_returns_none_when_machines_already_set_and_otherwise_current_tests.rs");
