//! Host-side unit tests for the routines faceted filter: the `StatusFacet` /
//! `MachineFacet` / `AgentFacet` codecs and the pure `RoutineFilter` matching +
//! list helpers that back the search box, status/agent/machine facets, and live
//! result count. All deterministic given a fixed `now`; no DOM/wasm dependency
//! (mirrors the `cron_jobs_tests.rs` conventions).

use super::*;
use chrono::{Local, TimeZone};

/// A fixed reference instant 30 s past the top of the hour so a top-of-hour
/// schedule's next fire (~59.5 m out) lands inside a 1-hour "due soon" window
/// but not a 5-minute one.
fn now() -> DateTime<Local> {
    Local.with_ymd_and_hms(2026, 6, 22, 12, 0, 30).unwrap()
}

fn window() -> Duration {
    Duration::seconds(DUE_SOON_WINDOW_SECS)
}

/// Build a minimal `Routine` with just the fields the filter reads.
fn routine(
    title: &str,
    agent: &str,
    schedule: &str,
    machines: &[&str],
    enabled: bool,
) -> Routine {
    Routine {
        id: title.into(),
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
        StatusFacet::All,
        StatusFacet::Enabled,
        StatusFacet::Disabled,
        StatusFacet::DueSoon,
    ] {
        assert_eq!(StatusFacet::from_str(f.as_str()), f);
    }
    assert_eq!(StatusFacet::from_str("nonsense"), StatusFacet::All);
    assert_eq!(StatusFacet::default(), StatusFacet::All);
}

#[test]
fn machine_facet_roundtrips_through_select_value() {
    let any = MachineFacet::Any;
    let unassigned = MachineFacet::Unassigned;
    let specific = MachineFacet::Machine("alpha".into());
    assert_eq!(MachineFacet::from_value(&any.as_value()), any);
    assert_eq!(MachineFacet::from_value(&unassigned.as_value()), unassigned);
    assert_eq!(MachineFacet::from_value(&specific.as_value()), specific);
    assert_eq!(MachineFacet::default(), MachineFacet::Any);
}

#[test]
fn machine_facet_decodes_a_plain_id_as_specific() {
    assert_eq!(
        MachineFacet::from_value("worker-1"),
        MachineFacet::Machine("worker-1".into())
    );
}

#[test]
fn agent_facet_roundtrips_through_select_value() {
    let all = AgentFacet::All;
    let specific = AgentFacet::Agent("claude".into());
    assert_eq!(AgentFacet::from_value(&all.as_value()), all);
    assert_eq!(AgentFacet::from_value(&specific.as_value()), specific);
    assert_eq!(AgentFacet::default(), AgentFacet::All);
}

#[test]
fn agent_facet_decodes_a_plain_name_as_specific() {
    assert_eq!(
        AgentFacet::from_value("codex"),
        AgentFacet::Agent("codex".into())
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
        status: StatusFacet::Enabled,
        ..Default::default()
    };
    assert!(s.is_active());

    let m = RoutineFilter {
        machine: MachineFacet::Unassigned,
        ..Default::default()
    };
    assert!(m.is_active());

    let a = RoutineFilter {
        agent: AgentFacet::Agent("claude".into()),
        ..Default::default()
    };
    assert!(a.is_active());
}

// ── Status facet matching ─────────────────────────────────────────────────────

#[test]
fn status_all_matches_regardless_of_enabled() {
    let f = RoutineFilter::default();
    assert!(f.matches(&routine("a", "c", "0 * * * *", &[], true), now(), window()));
    assert!(f.matches(&routine("b", "c", "0 * * * *", &[], false), now(), window()));
}

#[test]
fn status_enabled_and_disabled_partition() {
    let on = routine("a", "c", "0 * * * *", &[], true);
    let off = routine("b", "c", "0 * * * *", &[], false);
    let enabled = RoutineFilter {
        status: StatusFacet::Enabled,
        ..Default::default()
    };
    let disabled = RoutineFilter {
        status: StatusFacet::Disabled,
        ..Default::default()
    };
    assert!(enabled.matches(&on, now(), window()));
    assert!(!enabled.matches(&off, now(), window()));
    assert!(disabled.matches(&off, now(), window()));
    assert!(!disabled.matches(&on, now(), window()));
}

#[test]
fn status_due_soon_needs_enabled_and_an_imminent_fire() {
    let f = RoutineFilter {
        status: StatusFacet::DueSoon,
        ..Default::default()
    };
    // Enabled, fires at the next top of hour (~59.5 m) → inside the 1h window.
    let soon = routine("a", "c", "0 * * * *", &[], true);
    assert!(f.matches(&soon, now(), window()));
    // Same schedule but disabled → never "due".
    let soon_off = routine("b", "c", "0 * * * *", &[], false);
    assert!(!f.matches(&soon_off, now(), window()));
    // Enabled but far away (annually, Jan 1) → outside the window.
    let far = routine("c", "c", "0 0 1 1 *", &[], true);
    assert!(!f.matches(&far, now(), window()));
}

// ── Machine facet matching ────────────────────────────────────────────────────

#[test]
fn machine_any_matches_all() {
    let f = RoutineFilter::default();
    assert!(f.matches(&routine("a", "c", "0 * * * *", &["m1"], true), now(), window()));
    assert!(f.matches(&routine("b", "c", "0 * * * *", &[], true), now(), window()));
}

#[test]
fn machine_specific_requires_membership() {
    let f = RoutineFilter {
        machine: MachineFacet::Machine("m1".into()),
        ..Default::default()
    };
    assert!(f.matches(
        &routine("a", "c", "0 * * * *", &["m1", "m2"], true),
        now(),
        window()
    ));
    assert!(!f.matches(&routine("b", "c", "0 * * * *", &["m2"], true), now(), window()));
    assert!(!f.matches(&routine("c", "c", "0 * * * *", &[], true), now(), window()));
}

#[test]
fn machine_unassigned_matches_only_empty() {
    let f = RoutineFilter {
        machine: MachineFacet::Unassigned,
        ..Default::default()
    };
    assert!(f.matches(&routine("a", "c", "0 * * * *", &[], true), now(), window()));
    assert!(!f.matches(&routine("b", "c", "0 * * * *", &["m1"], true), now(), window()));
}

// ── Agent facet matching ──────────────────────────────────────────────────────

#[test]
fn agent_all_matches_any_agent() {
    let f = RoutineFilter::default();
    assert!(f.matches(&routine("a", "claude", "0 * * * *", &[], true), now(), window()));
    assert!(f.matches(&routine("b", "codex", "0 * * * *", &[], true), now(), window()));
}

#[test]
fn agent_specific_requires_exact_match() {
    let f = RoutineFilter {
        agent: AgentFacet::Agent("claude".into()),
        ..Default::default()
    };
    assert!(f.matches(&routine("a", "claude", "0 * * * *", &[], true), now(), window()));
    assert!(!f.matches(&routine("b", "codex", "0 * * * *", &[], true), now(), window()));
}

// ── Free-text matching ────────────────────────────────────────────────────────

#[test]
fn query_matches_across_title_agent_and_schedule() {
    let r = routine("nightly-triage", "claude", "0 3 * * *", &[], true);
    let by_title = RoutineFilter {
        query: "nightly".into(),
        ..Default::default()
    };
    let by_agent = RoutineFilter {
        query: "CLAUDE".into(),
        ..Default::default()
    };
    let by_schedule = RoutineFilter {
        query: "0 3".into(),
        ..Default::default()
    };
    assert!(by_title.matches(&r, now(), window()));
    assert!(by_agent.matches(&r, now(), window())); // case-insensitive
    assert!(by_schedule.matches(&r, now(), window()));
}

#[test]
fn query_matches_prompt_and_description() {
    let mut r = routine("r1", "claude", "0 3 * * *", &[], true);
    r.schedule_description = Some("Every day at 03:00".into());
    r.prompt = "Review open PRs and summarize findings".into();
    let by_desc = RoutineFilter {
        query: "every day".into(),
        ..Default::default()
    };
    let by_prompt = RoutineFilter {
        query: "open prs".into(),
        ..Default::default()
    };
    assert!(by_desc.matches(&r, now(), window()));
    assert!(by_prompt.matches(&r, now(), window()));
}

#[test]
fn query_is_trimmed_and_non_match_is_excluded() {
    let r = routine("alpha", "claude", "0 3 * * *", &[], true);
    let trimmed = RoutineFilter {
        query: "  alpha  ".into(),
        ..Default::default()
    };
    let miss = RoutineFilter {
        query: "zeta".into(),
        ..Default::default()
    };
    assert!(trimmed.matches(&r, now(), window()));
    assert!(!miss.matches(&r, now(), window()));
}

// ── Combined facets ───────────────────────────────────────────────────────────

#[test]
fn facets_and_together() {
    let f = RoutineFilter {
        query: "triage".into(),
        status: StatusFacet::Enabled,
        machine: MachineFacet::Machine("m1".into()),
        agent: AgentFacet::Agent("claude".into()),
    };
    let hit = routine("triage", "claude", "0 * * * *", &["m1"], true);
    let wrong_machine = routine("triage", "claude", "0 * * * *", &["m2"], true);
    let wrong_agent = routine("triage", "codex", "0 * * * *", &["m1"], true);
    let disabled = routine("triage", "claude", "0 * * * *", &["m1"], false);
    let wrong_text = routine("cleanup", "claude", "0 * * * *", &["m1"], true);
    assert!(f.matches(&hit, now(), window()));
    assert!(!f.matches(&wrong_machine, now(), window()));
    assert!(!f.matches(&wrong_agent, now(), window()));
    assert!(!f.matches(&disabled, now(), window()));
    assert!(!f.matches(&wrong_text, now(), window()));
}

// ── List helpers ──────────────────────────────────────────────────────────────

#[test]
fn filter_routines_preserves_order_and_narrows() {
    let routines = vec![
        routine("a", "c", "0 * * * *", &[], true),
        routine("b", "c", "0 * * * *", &[], false),
        routine("c", "c", "0 * * * *", &[], true),
    ];
    let f = RoutineFilter {
        status: StatusFacet::Enabled,
        ..Default::default()
    };
    let out = filter_routines(&routines, &f, now(), window());
    let ids: Vec<&str> = out.iter().map(|r| r.id.as_str()).collect();
    assert_eq!(ids, vec!["a", "c"]);
}

#[test]
fn filter_routines_unfiltered_returns_all() {
    let routines = vec![
        routine("a", "c", "0 * * * *", &[], true),
        routine("b", "c", "0 * * * *", &[], false),
    ];
    let out = filter_routines(&routines, &RoutineFilter::default(), now(), window());
    assert_eq!(out.len(), 2);
}

#[test]
fn distinct_agents_are_sorted_and_deduped() {
    let routines = vec![
        routine("a", "claude", "0 * * * *", &[], true),
        routine("b", "codex", "0 * * * *", &[], true),
        routine("c", "claude", "0 * * * *", &[], true),
    ];
    assert_eq!(distinct_agents(&routines), vec!["claude", "codex"]);
}

#[test]
fn distinct_machines_are_sorted_and_deduped() {
    let routines = vec![
        routine("a", "c", "0 * * * *", &["m2", "m1"], true),
        routine("b", "c", "0 * * * *", &["m1", "m3"], true),
        routine("c", "c", "0 * * * *", &[], true),
    ];
    assert_eq!(distinct_machines(&routines), vec!["m1", "m2", "m3"]);
}

#[test]
fn unassigned_count_tallies_dormant_routines() {
    let routines = vec![
        routine("a", "c", "0 * * * *", &["m1"], true),
        routine("b", "c", "0 * * * *", &[], true),
        routine("c", "c", "0 * * * *", &[], false),
    ];
    assert_eq!(unassigned_count(&routines), 2);
}
