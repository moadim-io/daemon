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
            known.iter().any(|a| a == spec.agent),
            "agent {:?} for routine {:?} is not a built-in agent",
            spec.agent,
            spec.title
        );
    }
}

#[test]
fn materialize_stamps_timestamps_and_marks_managed() {
    let spec = &DEFAULT_ROUTINES[0];
    let r = materialize(spec, 1234);
    assert_eq!(r.created_at, 1234);
    assert_eq!(r.updated_at, 1234);
    assert_eq!(r.source, "managed");
    assert!(r.enabled);
    assert!(r.last_triggered_at.is_none());
    assert!(!r.id.is_empty());
    // Schedule is normalized, not the raw spec string.
    assert_eq!(r.schedule, normalize_schedule(spec.schedule));
}

#[test]
fn materialize_assigns_unique_ids() {
    let spec = &DEFAULT_ROUTINES[0];
    assert_ne!(materialize(spec, 0).id, materialize(spec, 0).id);
}
