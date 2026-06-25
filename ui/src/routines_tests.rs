//! Host-side unit tests for the routines faceted filter: the `RoutineStatusFacet`,
//! `AgentFacet`, `RoutineMachineFacet` codecs and the pure `RoutineFilter` matching
//! + list helpers. No DOM/wasm dependency (mirrors the `cron_jobs_tests.rs` convention).

use super::*;

/// Build a routine with the fields the filter reads; the rest are inert.
fn routine(
    id: &str,
    title: &str,
    agent: &str,
    schedule: &str,
    machines: &[&str],
    repos: &[&str],
    enabled: bool,
) -> Routine {
    Routine {
        id: id.into(),
        title: title.into(),
        agent: agent.into(),
        schedule: schedule.into(),
        prompt: String::new(),
        repositories: repos
            .iter()
            .map(|r| Repository {
                repository: (*r).to_string(),
                branch: None,
            })
            .collect(),
        machines: machines.iter().map(|m| (*m).to_string()).collect(),
        enabled,
        source: String::new(),
        created_at: 0,
        updated_at: 0,
        last_manual_trigger_at: None,
        last_scheduled_trigger_at: None,
        ttl_secs: None,
        agent_registered: false,
        file_path: String::new(),
        schedule_description: None,
    }
}

/// Fixed deterministic "now" for tests (2026-01-01 12:00:00 local).
fn now() -> DateTime<Local> {
    Local.with_ymd_and_hms(2026, 1, 1, 12, 0, 0).unwrap()
}

/// DueSoon window matching `DUE_SOON_WINDOW_SECS`.
fn window() -> Duration {
    Duration::seconds(DUE_SOON_WINDOW_SECS)
}

// ── RoutineStatusFacet codecs ─────────────────────────────────────────────────

#[test]
fn status_facet_roundtrips_and_defaults_to_all() {
    for f in [
        RoutineStatusFacet::All,
        RoutineStatusFacet::Enabled,
        RoutineStatusFacet::Disabled,
        RoutineStatusFacet::Dormant,
        RoutineStatusFacet::DueSoon,
    ] {
        assert_eq!(RoutineStatusFacet::from_str(f.as_str()), f);
    }
    assert_eq!(
        RoutineStatusFacet::from_str("nonsense"),
        RoutineStatusFacet::All
    );
    assert_eq!(RoutineStatusFacet::default(), RoutineStatusFacet::All);
}

// ── AgentFacet codecs ─────────────────────────────────────────────────────────

#[test]
fn agent_facet_roundtrips_and_defaults_to_all() {
    let all = AgentFacet::All;
    let named = AgentFacet::Named("claude".into());
    assert_eq!(AgentFacet::from_value(&all.as_value()), all);
    assert_eq!(AgentFacet::from_value(&named.as_value()), named);
    assert_eq!(AgentFacet::default(), AgentFacet::All);
}

#[test]
fn agent_facet_decodes_a_plain_name_as_named() {
    assert_eq!(
        AgentFacet::from_value("codex"),
        AgentFacet::Named("codex".into())
    );
}

// ── RoutineMachineFacet codecs ────────────────────────────────────────────────

#[test]
fn machine_facet_roundtrips_through_select_value() {
    let any = RoutineMachineFacet::Any;
    let unassigned = RoutineMachineFacet::Unassigned;
    let specific = RoutineMachineFacet::Machine("alpha".into());
    assert_eq!(RoutineMachineFacet::from_value(&any.as_value()), any);
    assert_eq!(
        RoutineMachineFacet::from_value(&unassigned.as_value()),
        unassigned
    );
    assert_eq!(
        RoutineMachineFacet::from_value(&specific.as_value()),
        specific
    );
    assert_eq!(RoutineMachineFacet::default(), RoutineMachineFacet::Any);
}

#[test]
fn machine_facet_decodes_a_plain_id_as_specific() {
    assert_eq!(
        RoutineMachineFacet::from_value("worker-1"),
        RoutineMachineFacet::Machine("worker-1".into())
    );
}

// ── is_active ─────────────────────────────────────────────────────────────────

#[test]
fn default_filter_is_inactive() {
    assert!(!RoutineFilter::default().is_active());
}

#[test]
fn is_active_detects_each_facet() {
    let q = RoutineFilter {
        query: "  x ".into(),
        ..Default::default()
    };
    assert!(q.is_active());
    // Whitespace-only query is not active.
    let blank = RoutineFilter {
        query: "   ".into(),
        ..Default::default()
    };
    assert!(!blank.is_active());

    let s = RoutineFilter {
        status: RoutineStatusFacet::Enabled,
        ..Default::default()
    };
    assert!(s.is_active());

    let due = RoutineFilter {
        status: RoutineStatusFacet::DueSoon,
        ..Default::default()
    };
    assert!(due.is_active());

    let a = RoutineFilter {
        agent: AgentFacet::Named("claude".into()),
        ..Default::default()
    };
    assert!(a.is_active());

    let m = RoutineFilter {
        machine: RoutineMachineFacet::Unassigned,
        ..Default::default()
    };
    assert!(m.is_active());
}

// ── Status facet matching ─────────────────────────────────────────────────────

#[test]
fn status_all_matches_regardless_of_enabled() {
    let f = RoutineFilter::default();
    let on = routine("a", "t", "claude", "0 * * * *", &["m1"], &[], true);
    let off = routine("b", "t", "claude", "0 * * * *", &["m1"], &[], false);
    assert!(f.matches(&on, now(), window()));
    assert!(f.matches(&off, now(), window()));
}

#[test]
fn status_enabled_and_disabled_partition() {
    let on = routine("a", "t", "claude", "0 * * * *", &["m1"], &[], true);
    let off = routine("b", "t", "claude", "0 * * * *", &["m1"], &[], false);
    let enabled = RoutineFilter {
        status: RoutineStatusFacet::Enabled,
        ..Default::default()
    };
    let disabled = RoutineFilter {
        status: RoutineStatusFacet::Disabled,
        ..Default::default()
    };
    assert!(enabled.matches(&on, now(), window()));
    assert!(!enabled.matches(&off, now(), window()));
    assert!(disabled.matches(&off, now(), window()));
    assert!(!disabled.matches(&on, now(), window()));
}

#[test]
fn status_dormant_requires_enabled_and_no_machines() {
    let f = RoutineFilter {
        status: RoutineStatusFacet::Dormant,
        ..Default::default()
    };
    // Enabled, no machines → dormant.
    let dormant = routine("a", "t", "claude", "0 * * * *", &[], &[], true);
    assert!(f.matches(&dormant, now(), window()));
    // Enabled WITH machines → not dormant.
    let active = routine("b", "t", "claude", "0 * * * *", &["m1"], &[], true);
    assert!(!f.matches(&active, now(), window()));
    // Disabled, no machines → also not dormant (disabled, not "waiting for machines").
    let disabled_no_machine = routine("c", "t", "claude", "0 * * * *", &[], &[], false);
    assert!(!f.matches(&disabled_no_machine, now(), window()));
}

#[test]
fn status_due_soon_matches_enabled_routines_firing_within_window() {
    let f = RoutineFilter {
        status: RoutineStatusFacet::DueSoon,
        ..Default::default()
    };
    // `* * * * *` fires every minute — always within a 1-hour window.
    let imminent = routine("a", "t", "claude", "* * * * *", &["m1"], &[], true);
    assert!(f.matches(&imminent, now(), window()));

    // Disabled, even if schedule would fire: not due soon.
    let disabled = routine("b", "t", "claude", "* * * * *", &["m1"], &[], false);
    assert!(!f.matches(&disabled, now(), window()));

    // Schedule that fires at minute 0 of every hour; from 12:00:00, next fire
    // is 13:00:00 (60 min), which equals the 1-hour window boundary —
    // `fires_within` checks `then - now <= window`, so 60 min = 3600 s ≤ 3600 s → true.
    let boundary = routine("c", "t", "claude", "0 * * * *", &["m1"], &[], true);
    assert!(f.matches(&boundary, now(), window()));

    // Invalid / empty schedule → never fires → not due soon.
    let never = routine("d", "t", "claude", "", &["m1"], &[], true);
    assert!(!f.matches(&never, now(), window()));
}

// ── Agent facet matching ──────────────────────────────────────────────────────

#[test]
fn agent_all_matches_any_agent() {
    let f = RoutineFilter::default();
    let c = routine("a", "t", "claude", "0 * * * *", &["m1"], &[], true);
    let cx = routine("b", "t", "codex", "0 * * * *", &["m1"], &[], true);
    assert!(f.matches(&c, now(), window()));
    assert!(f.matches(&cx, now(), window()));
}

#[test]
fn agent_named_filters_by_exact_agent() {
    let f = RoutineFilter {
        agent: AgentFacet::Named("claude".into()),
        ..Default::default()
    };
    let claude = routine("a", "t", "claude", "0 * * * *", &["m1"], &[], true);
    let codex = routine("b", "t", "codex", "0 * * * *", &["m1"], &[], true);
    assert!(f.matches(&claude, now(), window()));
    assert!(!f.matches(&codex, now(), window()));
}

// ── Machine facet matching ────────────────────────────────────────────────────

#[test]
fn machine_any_matches_regardless_of_machines() {
    let f = RoutineFilter::default();
    let with = routine("a", "t", "claude", "0 * * * *", &["m1"], &[], true);
    let without = routine("b", "t", "claude", "0 * * * *", &[], &[], true);
    assert!(f.matches(&with, now(), window()));
    assert!(f.matches(&without, now(), window()));
}

#[test]
fn machine_unassigned_matches_only_empty_machines() {
    let f = RoutineFilter {
        machine: RoutineMachineFacet::Unassigned,
        ..Default::default()
    };
    let with = routine("a", "t", "claude", "0 * * * *", &["m1"], &[], true);
    let without = routine("b", "t", "claude", "0 * * * *", &[], &[], true);
    assert!(!f.matches(&with, now(), window()));
    assert!(f.matches(&without, now(), window()));
}

#[test]
fn machine_specific_matches_only_that_machine() {
    let f = RoutineFilter {
        machine: RoutineMachineFacet::Machine("m1".into()),
        ..Default::default()
    };
    let m1 = routine("a", "t", "claude", "0 * * * *", &["m1", "m2"], &[], true);
    let m2_only = routine("b", "t", "claude", "0 * * * *", &["m2"], &[], true);
    let none = routine("c", "t", "claude", "0 * * * *", &[], &[], true);
    assert!(f.matches(&m1, now(), window()));
    assert!(!f.matches(&m2_only, now(), window()));
    assert!(!f.matches(&none, now(), window()));
}

// ── Free-text search ──────────────────────────────────────────────────────────

#[test]
fn query_matches_title() {
    let f = RoutineFilter {
        query: "deploy".into(),
        ..Default::default()
    };
    let hit = routine("a", "Deploy prod", "claude", "0 * * * *", &[], &[], true);
    let miss = routine("b", "Build images", "claude", "0 * * * *", &[], &[], true);
    assert!(f.matches(&hit, now(), window()));
    assert!(!f.matches(&miss, now(), window()));
}

#[test]
fn query_matches_agent() {
    let f = RoutineFilter {
        query: "codex".into(),
        ..Default::default()
    };
    let hit = routine("a", "t", "codex", "0 * * * *", &[], &[], true);
    let miss = routine("b", "t", "claude", "0 * * * *", &[], &[], true);
    assert!(f.matches(&hit, now(), window()));
    assert!(!f.matches(&miss, now(), window()));
}

#[test]
fn query_matches_repository_url() {
    let f = RoutineFilter {
        query: "github.com/acme".into(),
        ..Default::default()
    };
    let hit = routine(
        "a",
        "t",
        "claude",
        "0 * * * *",
        &[],
        &["https://github.com/acme/backend"],
        true,
    );
    let miss = routine(
        "b",
        "t",
        "claude",
        "0 * * * *",
        &[],
        &["https://github.com/other/foo"],
        true,
    );
    assert!(f.matches(&hit, now(), window()));
    assert!(!f.matches(&miss, now(), window()));
}

#[test]
fn query_is_case_insensitive() {
    let f = RoutineFilter {
        query: "DEPLOY".into(),
        ..Default::default()
    };
    let hit = routine("a", "deploy staging", "claude", "0 * * * *", &[], &[], true);
    assert!(f.matches(&hit, now(), window()));
}

#[test]
fn empty_query_matches_all() {
    let f = RoutineFilter {
        query: "   ".into(),
        ..Default::default()
    };
    let r = routine("a", "anything", "claude", "0 * * * *", &["m"], &[], true);
    assert!(f.matches(&r, now(), window()));
}

// ── filter_routines helper ────────────────────────────────────────────────────

#[test]
fn filter_routines_returns_only_matching() {
    let routines = vec![
        routine("a", "alpha", "claude", "0 * * * *", &["m1"], &[], true),
        routine("b", "beta", "codex", "0 * * * *", &["m1"], &[], false),
        routine("c", "gamma", "claude", "0 * * * *", &[], &[], true),
    ];
    let f = RoutineFilter {
        status: RoutineStatusFacet::Enabled,
        ..Default::default()
    };
    let got = filter_routines(&routines, &f, now(), window());
    assert_eq!(got.len(), 2);
    assert!(got.iter().all(|r| r.enabled));
}

#[test]
fn filter_routines_due_soon_returns_imminent_enabled_only() {
    let routines = vec![
        // fires every minute → always due soon
        routine("a", "frequent", "claude", "* * * * *", &["m1"], &[], true),
        // fires hourly; from 12:00 next is 13:00 → within window
        routine("b", "hourly", "claude", "0 * * * *", &["m1"], &[], true),
        // disabled, same schedule — excluded
        routine("c", "off", "claude", "* * * * *", &["m1"], &[], false),
    ];
    let f = RoutineFilter {
        status: RoutineStatusFacet::DueSoon,
        ..Default::default()
    };
    let got = filter_routines(&routines, &f, now(), window());
    assert_eq!(got.len(), 2);
    assert!(got.iter().all(|r| r.enabled));
}

// ── distinct helpers ──────────────────────────────────────────────────────────

#[test]
fn distinct_agents_returns_sorted_unique_agents() {
    let routines = vec![
        routine("a", "t", "codex", "0 * * * *", &[], &[], true),
        routine("b", "t", "claude", "0 * * * *", &[], &[], true),
        routine("c", "t", "claude", "0 * * * *", &[], &[], true),
    ];
    let agents = distinct_agents(&routines);
    assert_eq!(agents, vec!["claude", "codex"]);
}

#[test]
fn distinct_machines_r_returns_sorted_unique_machines() {
    let routines = vec![
        routine("a", "t", "claude", "0 * * * *", &["m2", "m1"], &[], true),
        routine("b", "t", "claude", "0 * * * *", &["m1", "m3"], &[], true),
    ];
    let machines = distinct_machines_r(&routines);
    assert_eq!(machines, vec!["m1", "m2", "m3"]);
}

#[test]
fn unassigned_routines_count_counts_empty_machine_lists() {
    let routines = vec![
        routine("a", "t", "claude", "0 * * * *", &[], &[], true),
        routine("b", "t", "claude", "0 * * * *", &["m1"], &[], true),
        routine("c", "t", "claude", "0 * * * *", &[], &[], false),
    ];
    assert_eq!(unassigned_routines_count(&routines), 2);
}

// ── Selection state transitions ───────────────────────────────────────────────

fn make_state(ids: &[&str]) -> RState {
    let routines = ids
        .iter()
        .map(|id| routine(id, "t", "claude", "0 * * * *", &[], &[], true))
        .collect();
    RState {
        routines,
        ..RState::default()
    }
}

#[test]
fn select_only_clears_others_and_sets_anchor() {
    use std::rc::Rc;
    let mut s = make_state(&["a", "b", "c"]);
    s.selected.insert("b".into());
    let s = RState::reduce(
        Rc::new(s),
        RAction::SelectRoutine {
            id: "a".into(),
            kind: SelectKind::Only,
        },
    );
    assert_eq!(s.selected.len(), 1);
    assert!(s.selected.contains("a"));
    assert_eq!(s.select_anchor.as_deref(), Some("a"));
}

#[test]
fn select_toggle_adds_when_absent_and_removes_when_present() {
    use std::rc::Rc;
    let s = make_state(&["a", "b"]);
    let s = RState::reduce(
        Rc::new(s),
        RAction::SelectRoutine {
            id: "a".into(),
            kind: SelectKind::Toggle,
        },
    );
    assert!(s.selected.contains("a"));

    let s = RState::reduce(
        s,
        RAction::SelectRoutine {
            id: "a".into(),
            kind: SelectKind::Toggle,
        },
    );
    assert!(!s.selected.contains("a"));
}

#[test]
fn select_range_covers_contiguous_slice() {
    use std::rc::Rc;
    let mut s = make_state(&["a", "b", "c", "d"]);
    // Anchor at "a".
    s = (*RState::reduce(
        Rc::new(s),
        RAction::SelectRoutine {
            id: "a".into(),
            kind: SelectKind::Only,
        },
    ))
    .clone();
    // Shift+click "c" — should select a, b, c.
    let s = RState::reduce(
        Rc::new(s),
        RAction::SelectRoutine {
            id: "c".into(),
            kind: SelectKind::Range,
        },
    );
    assert!(s.selected.contains("a"));
    assert!(s.selected.contains("b"));
    assert!(s.selected.contains("c"));
    assert!(!s.selected.contains("d"));
}

#[test]
fn select_all_selects_given_ids() {
    use std::rc::Rc;
    let s = make_state(&["a", "b", "c"]);
    let s = RState::reduce(
        Rc::new(s),
        RAction::SelectAll(vec!["a".into(), "c".into()]),
    );
    assert_eq!(s.selected.len(), 2);
    assert!(s.selected.contains("a"));
    assert!(s.selected.contains("c"));
    assert!(!s.selected.contains("b"));
}

#[test]
fn clear_selection_empties_set_and_anchor() {
    use std::rc::Rc;
    let mut s = make_state(&["a", "b"]);
    s.selected.insert("a".into());
    s.select_anchor = Some("a".into());
    let s = RState::reduce(Rc::new(s), RAction::ClearSelection);
    assert!(s.selected.is_empty());
    assert!(s.select_anchor.is_none());
}

#[test]
fn remove_many_drops_from_selection_and_routines() {
    use std::rc::Rc;
    let mut s = make_state(&["a", "b", "c"]);
    s.selected.insert("a".into());
    s.selected.insert("b".into());
    let s = RState::reduce(
        Rc::new(s),
        RAction::RemoveMany(vec!["a".into(), "b".into()]),
    );
    assert_eq!(s.routines.len(), 1);
    assert_eq!(s.routines[0].id, "c");
    assert!(s.selected.is_empty());
}

#[test]
fn loaded_drops_stale_selections() {
    use std::rc::Rc;
    let mut s = make_state(&["a", "b"]);
    s.selected.insert("a".into());
    s.selected.insert("gone".into()); // no longer in the new list
    let new_routines = vec![
        routine("a", "t", "claude", "0 * * * *", &[], &[], true),
        routine("c", "t", "claude", "0 * * * *", &[], &[], true),
    ];
    let s = RState::reduce(Rc::new(s), RAction::Loaded(new_routines));
    assert!(s.selected.contains("a"));
    assert!(!s.selected.contains("gone"));
}
