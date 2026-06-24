//! Host-side unit tests for the routines faceted filter: the `RoutineStatusFacet` /
//! `AgentFacet` / `RoutineMachineFacet` codecs and the pure `RoutineFilter` matching
//! + list helpers that back the search box, status/agent/machine facets, and live
//! result count. No DOM/wasm dependency (mirrors the `cron_jobs_tests.rs` conventions).

use super::*;

/// Build a routine with the fields the filter reads; the rest are inert.
fn routine(
    title: &str,
    agent: &str,
    schedule: &str,
    machines: &[&str],
    enabled: bool,
) -> Routine {
    Routine {
        id: title.to_lowercase().replace(' ', "-"),
        schedule: schedule.into(),
        title: title.into(),
        agent: agent.into(),
        prompt: String::new(),
        repositories: vec![],
        machines: machines.iter().map(|m| (*m).to_string()).collect(),
        enabled,
        source: String::new(),
        created_at: 0,
        updated_at: 0,
        last_manual_trigger_at: None,
        ttl_secs: None,
        agent_registered: true,
        file_path: String::new(),
        schedule_description: None,
    }
}

// ── Facet codecs ──────────────────────────────────────────────────────────────

#[test]
fn status_facet_roundtrips_and_defaults_to_all() {
    for f in [
        RoutineStatusFacet::All,
        RoutineStatusFacet::Enabled,
        RoutineStatusFacet::Disabled,
    ] {
        assert_eq!(RoutineStatusFacet::from_str(f.as_str()), f);
    }
    assert_eq!(RoutineStatusFacet::from_str("nonsense"), RoutineStatusFacet::All);
    assert_eq!(RoutineStatusFacet::default(), RoutineStatusFacet::All);
}

#[test]
fn agent_facet_roundtrips_and_defaults_to_any() {
    assert_eq!(AgentFacet::from_value(&AgentFacet::Any.as_value()), AgentFacet::Any);
    let specific = AgentFacet::Agent("claude".into());
    assert_eq!(AgentFacet::from_value(&specific.as_value()), specific);
    assert_eq!(AgentFacet::default(), AgentFacet::Any);
}

#[test]
fn machine_facet_roundtrips_and_defaults_to_any() {
    for f in [
        RoutineMachineFacet::Any,
        RoutineMachineFacet::Unassigned,
        RoutineMachineFacet::Machine("prod-1".into()),
    ] {
        assert_eq!(RoutineMachineFacet::from_value(&f.as_value()), f);
    }
    assert_eq!(RoutineMachineFacet::default(), RoutineMachineFacet::Any);
}

// ── RoutineFilter::is_active ──────────────────────────────────────────────────

#[test]
fn default_filter_is_not_active() {
    assert!(!RoutineFilter::default().is_active());
}

#[test]
fn filter_is_active_when_query_set() {
    let f = RoutineFilter {
        query: "foo".into(),
        ..RoutineFilter::default()
    };
    assert!(f.is_active());
}

#[test]
fn filter_is_active_when_status_not_all() {
    let f = RoutineFilter {
        status: RoutineStatusFacet::Enabled,
        ..RoutineFilter::default()
    };
    assert!(f.is_active());
}

#[test]
fn filter_is_active_when_agent_not_any() {
    let f = RoutineFilter {
        agent: AgentFacet::Agent("codex".into()),
        ..RoutineFilter::default()
    };
    assert!(f.is_active());
}

#[test]
fn filter_is_active_when_machine_not_any() {
    let f = RoutineFilter {
        machine: RoutineMachineFacet::Unassigned,
        ..RoutineFilter::default()
    };
    assert!(f.is_active());
}

// ── RoutineFilter::matches — status facet ────────────────────────────────────

#[test]
fn status_all_passes_both() {
    let enabled = routine("A", "claude", "0 * * * *", &["m1"], true);
    let disabled = routine("B", "claude", "0 * * * *", &["m1"], false);
    let f = RoutineFilter::default();
    assert!(f.matches(&enabled));
    assert!(f.matches(&disabled));
}

#[test]
fn status_enabled_excludes_disabled() {
    let r = routine("A", "claude", "0 * * * *", &["m1"], false);
    let f = RoutineFilter {
        status: RoutineStatusFacet::Enabled,
        ..RoutineFilter::default()
    };
    assert!(!f.matches(&r));
}

#[test]
fn status_disabled_excludes_enabled() {
    let r = routine("A", "claude", "0 * * * *", &["m1"], true);
    let f = RoutineFilter {
        status: RoutineStatusFacet::Disabled,
        ..RoutineFilter::default()
    };
    assert!(!f.matches(&r));
}

// ── RoutineFilter::matches — agent facet ─────────────────────────────────────

#[test]
fn agent_any_passes_all() {
    let r = routine("A", "codex", "0 * * * *", &[], true);
    let f = RoutineFilter::default();
    assert!(f.matches(&r));
}

#[test]
fn agent_specific_filters_by_agent_name() {
    let claude = routine("A", "claude", "0 * * * *", &[], true);
    let codex = routine("B", "codex", "0 * * * *", &[], true);
    let f = RoutineFilter {
        agent: AgentFacet::Agent("claude".into()),
        ..RoutineFilter::default()
    };
    assert!(f.matches(&claude));
    assert!(!f.matches(&codex));
}

// ── RoutineFilter::matches — machine facet ───────────────────────────────────

#[test]
fn machine_any_passes_all() {
    let r = routine("A", "claude", "0 * * * *", &[], true);
    let f = RoutineFilter::default();
    assert!(f.matches(&r));
}

#[test]
fn machine_unassigned_excludes_assigned() {
    let assigned = routine("A", "claude", "0 * * * *", &["m1"], true);
    let f = RoutineFilter {
        machine: RoutineMachineFacet::Unassigned,
        ..RoutineFilter::default()
    };
    assert!(!f.matches(&assigned));
}

#[test]
fn machine_unassigned_passes_no_machine() {
    let unassigned = routine("A", "claude", "0 * * * *", &[], true);
    let f = RoutineFilter {
        machine: RoutineMachineFacet::Unassigned,
        ..RoutineFilter::default()
    };
    assert!(f.matches(&unassigned));
}

#[test]
fn machine_specific_filters_membership() {
    let on_m1 = routine("A", "claude", "0 * * * *", &["m1", "m2"], true);
    let on_m3 = routine("B", "claude", "0 * * * *", &["m3"], true);
    let f = RoutineFilter {
        machine: RoutineMachineFacet::Machine("m1".into()),
        ..RoutineFilter::default()
    };
    assert!(f.matches(&on_m1));
    assert!(!f.matches(&on_m3));
}

// ── RoutineFilter::matches — free-text query ─────────────────────────────────

#[test]
fn query_matches_title_case_insensitive() {
    let r = routine("Weekly Report", "claude", "0 9 * * 1", &[], true);
    let f = RoutineFilter {
        query: "weekly".into(),
        ..RoutineFilter::default()
    };
    assert!(f.matches(&r));
}

#[test]
fn query_matches_agent_name() {
    let r = routine("My Routine", "codex", "0 * * * *", &[], true);
    let f = RoutineFilter {
        query: "codex".into(),
        ..RoutineFilter::default()
    };
    assert!(f.matches(&r));
}

#[test]
fn query_matches_schedule_string() {
    let r = routine("Nightly Job", "claude", "0 2 * * *", &[], true);
    let f = RoutineFilter {
        query: "0 2".into(),
        ..RoutineFilter::default()
    };
    assert!(f.matches(&r));
}

#[test]
fn query_no_match_returns_false() {
    let r = routine("Daily Backup", "claude", "0 1 * * *", &[], true);
    let f = RoutineFilter {
        query: "weekly".into(),
        ..RoutineFilter::default()
    };
    assert!(!f.matches(&r));
}

#[test]
fn empty_query_matches_all() {
    let r = routine("Anything", "codex", "0 * * * *", &[], true);
    let f = RoutineFilter {
        query: "   ".into(),
        ..RoutineFilter::default()
    };
    assert!(f.matches(&r));
}

// ── filter_routines list helper ───────────────────────────────────────────────

#[test]
fn filter_routines_preserves_order_and_excludes_non_matches() {
    let rs = vec![
        routine("Claude Weekly", "claude", "0 9 * * 1", &["m1"], true),
        routine("Codex Nightly", "codex", "0 2 * * *", &["m2"], false),
        routine("Claude Daily", "claude", "0 7 * * *", &["m1"], true),
    ];
    let f = RoutineFilter {
        agent: AgentFacet::Agent("claude".into()),
        status: RoutineStatusFacet::Enabled,
        ..RoutineFilter::default()
    };
    let result = filter_routines(&rs, &f);
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].title, "Claude Weekly");
    assert_eq!(result[1].title, "Claude Daily");
}

// ── distinct_agents ───────────────────────────────────────────────────────────

#[test]
fn distinct_agents_returns_sorted_unique_names() {
    let rs = vec![
        routine("A", "codex", "0 * * * *", &[], true),
        routine("B", "claude", "0 * * * *", &[], true),
        routine("C", "codex", "0 * * * *", &[], true),
    ];
    assert_eq!(distinct_agents(&rs), vec!["claude", "codex"]);
}

#[test]
fn distinct_agents_skips_empty_agent() {
    let mut r = routine("A", "", "0 * * * *", &[], true);
    r.agent = String::new();
    assert_eq!(distinct_agents(&[r]), Vec::<String>::new());
}

// ── distinct_machines_r / unassigned_count_r ─────────────────────────────────

#[test]
fn distinct_machines_r_returns_sorted_unique() {
    let rs = vec![
        routine("A", "claude", "0 * * * *", &["m2", "m1"], true),
        routine("B", "claude", "0 * * * *", &["m1", "m3"], true),
    ];
    assert_eq!(distinct_machines_r(&rs), vec!["m1", "m2", "m3"]);
}

#[test]
fn unassigned_count_r_counts_no_machine_routines() {
    let rs = vec![
        routine("A", "claude", "0 * * * *", &[], true),
        routine("B", "claude", "0 * * * *", &["m1"], true),
        routine("C", "claude", "0 * * * *", &[], false),
    ];
    assert_eq!(unassigned_count_r(&rs), 2);
}
