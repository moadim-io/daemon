#![allow(clippy::missing_docs_in_private_items)]

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
    assert!(routine.last_triggered_at.is_none());
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
