//! Host-side unit tests for the saved-view snapshot codec: `capture`/`decode`
//! round-tripping and graceful fallback on unknown/missing tokens. No DOM/wasm
//! dependency (mirrors the `refresh_tests.rs` conventions) — the `localStorage`
//! round-trip and the `SavedViewsBar` component require a browser and aren't
//! covered here.

use super::*;

#[test]
fn capture_decode_round_trips_default_state() {
    let filter = RoutineFilter::default();
    let snapshot = ViewSnapshot::capture(&filter, None, RDir::default(), RGroupBy::default());
    let (decoded_filter, sort_col, sort_dir, group_by) = decode(&snapshot);
    assert_eq!(decoded_filter, filter);
    assert_eq!(sort_col, None);
    assert_eq!(sort_dir, RDir::default());
    assert_eq!(group_by, RGroupBy::default());
}

#[test]
fn capture_decode_round_trips_populated_state() {
    let filter = RoutineFilter {
        query: "deploy".to_string(),
        status: RoutineStatusFacet::Snoozed,
        agent: AgentFacet::Named("claude".to_string()),
        machine: RoutineMachineFacet::Machine("box-1".to_string()),
        repository: RepositoryFacet::Named("org/repo".to_string()),
        tag: TagFacet::Named("nightly".to_string()),
    };
    let snapshot = ViewSnapshot::capture(&filter, Some(RCol::Health), RDir::Desc, RGroupBy::Agent);
    let (decoded_filter, sort_col, sort_dir, group_by) = decode(&snapshot);
    assert_eq!(decoded_filter, filter);
    assert_eq!(sort_col, Some(RCol::Health));
    assert_eq!(sort_dir, RDir::Desc);
    assert_eq!(group_by, RGroupBy::Agent);
}

#[test]
fn decode_falls_back_to_defaults_for_unknown_tokens() {
    let snapshot = ViewSnapshot {
        query: String::new(),
        status: "not-a-real-status".to_string(),
        agent: "\u{0}bogus".to_string(),
        machine: "some-machine".to_string(),
        repository: "\u{0}bogus".to_string(),
        tag: "\u{0}bogus".to_string(),
        sort_col: Some("not-a-real-col".to_string()),
        sort_dir: "sideways".to_string(),
        group_by: "not-a-real-group".to_string(),
    };
    let (filter, sort_col, sort_dir, group_by) = decode(&snapshot);
    assert_eq!(filter.status, RoutineStatusFacet::All);
    assert_eq!(
        filter.machine,
        RoutineMachineFacet::Machine("some-machine".to_string())
    );
    assert_eq!(sort_col, None);
    assert_eq!(sort_dir, RDir::Asc);
    assert_eq!(group_by, RGroupBy::None);
}

#[test]
fn decode_missing_sort_col_yields_none() {
    let snapshot = ViewSnapshot {
        sort_col: None,
        ..ViewSnapshot::default()
    };
    let (_, sort_col, _, _) = decode(&snapshot);
    assert_eq!(sort_col, None);
}
