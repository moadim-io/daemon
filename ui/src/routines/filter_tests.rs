use super::super::model::Repository;
use super::*;
use chrono::TimeZone;

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
        model: None,
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
        snoozed_until: None,
        skip_runs: None,
        power_saving: false,
        ttl_secs: None,
        tags: vec![],
        agent_registered: false,
        file_path: String::new(),
        schedule_description: None,
        goal: None,
        flag_count: 0,
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
        RoutineStatusFacet::Snoozed,
        RoutineStatusFacet::HasFlags,
        RoutineStatusFacet::AgentUnregistered,
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

// ── RepositoryFacet codecs ─────────────────────────────────────────────────────

#[test]
fn repository_facet_roundtrips_and_defaults_to_all() {
    let all = RepositoryFacet::All;
    let named = RepositoryFacet::Named("github.com/org/repo".into());
    assert_eq!(RepositoryFacet::from_value(&all.as_value()), all);
    assert_eq!(RepositoryFacet::from_value(&named.as_value()), named);
    assert_eq!(RepositoryFacet::default(), RepositoryFacet::All);
}

#[test]
fn repository_facet_decodes_a_plain_url_as_named() {
    assert_eq!(
        RepositoryFacet::from_value("github.com/org/repo"),
        RepositoryFacet::Named("github.com/org/repo".into())
    );
}

// ── TagFacet codecs ────────────────────────────────────────────────────────────

#[test]
fn tag_facet_roundtrips_and_defaults_to_all() {
    let all = TagFacet::All;
    let named = TagFacet::Named("nightly".into());
    assert_eq!(TagFacet::from_value(&all.as_value()), all);
    assert_eq!(TagFacet::from_value(&named.as_value()), named);
    assert_eq!(TagFacet::default(), TagFacet::All);
}

#[test]
fn tag_facet_decodes_a_plain_value_as_named() {
    assert_eq!(
        TagFacet::from_value("nightly"),
        TagFacet::Named("nightly".into())
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

    let r = RoutineFilter {
        repository: RepositoryFacet::Named("github.com/org/repo".into()),
        ..Default::default()
    };
    assert!(r.is_active());

    let t = RoutineFilter {
        tag: TagFacet::Named("nightly".into()),
        ..Default::default()
    };
    assert!(t.is_active());
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

#[test]
fn status_snoozed_matches_only_snoozed_routines() {
    let f = RoutineFilter {
        status: RoutineStatusFacet::Snoozed,
        ..Default::default()
    };
    let snoozed = Routine {
        snoozed_until: Some((now() + Duration::hours(1)).timestamp() as u64),
        ..routine("a", "t", "claude", "0 * * * *", &["m1"], &[], true)
    };
    let active = routine("b", "t", "claude", "0 * * * *", &["m1"], &[], true);
    let disabled_snoozed = Routine {
        snoozed_until: Some((now() + Duration::hours(1)).timestamp() as u64),
        ..routine("c", "t", "claude", "0 * * * *", &["m1"], &[], false)
    };
    assert!(f.matches(&snoozed, now(), window()));
    assert!(!f.matches(&active, now(), window()));
    // Disabled+snoozed: snoozed filter does not check enabled state.
    assert!(f.matches(&disabled_snoozed, now(), window()));
}

#[test]
fn status_has_flags_matches_only_flagged_routines() {
    let f = RoutineFilter {
        status: RoutineStatusFacet::HasFlags,
        ..Default::default()
    };
    let flagged = Routine {
        flag_count: 2,
        ..routine("a", "t", "claude", "0 * * * *", &["m1"], &[], true)
    };
    let clean = routine("b", "t", "claude", "0 * * * *", &["m1"], &[], true);
    assert!(f.matches(&flagged, now(), window()));
    assert!(!f.matches(&clean, now(), window()));
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

// ── Repository facet matching ─────────────────────────────────────────────────

#[test]
fn repository_all_matches_regardless_of_repositories() {
    let f = RoutineFilter::default();
    let with = routine("a", "t", "claude", "0 * * * *", &[], &["repo-a"], true);
    let without = routine("b", "t", "claude", "0 * * * *", &[], &[], true);
    assert!(f.matches(&with, now(), window()));
    assert!(f.matches(&without, now(), window()));
}

#[test]
fn repository_named_matches_only_routines_listing_that_repository() {
    let f = RoutineFilter {
        repository: RepositoryFacet::Named("repo-a".into()),
        ..Default::default()
    };
    let hit = routine(
        "a",
        "t",
        "claude",
        "0 * * * *",
        &[],
        &["repo-a", "repo-b"],
        true,
    );
    let other = routine("b", "t", "claude", "0 * * * *", &[], &["repo-b"], true);
    let none = routine("c", "t", "claude", "0 * * * *", &[], &[], true);
    assert!(f.matches(&hit, now(), window()));
    assert!(!f.matches(&other, now(), window()));
    assert!(!f.matches(&none, now(), window()));
}

// ── Tag facet matching ────────────────────────────────────────────────────────

#[test]
fn tag_all_matches_regardless_of_tags() {
    let f = RoutineFilter::default();
    let with = Routine {
        tags: vec!["nightly".into()],
        ..routine("a", "t", "claude", "0 * * * *", &[], &[], true)
    };
    let without = routine("b", "t", "claude", "0 * * * *", &[], &[], true);
    assert!(f.matches(&with, now(), window()));
    assert!(f.matches(&without, now(), window()));
}

#[test]
fn tag_named_matches_only_routines_carrying_that_tag() {
    let f = RoutineFilter {
        tag: TagFacet::Named("nightly".into()),
        ..Default::default()
    };
    let hit = Routine {
        tags: vec!["nightly".into(), "prod".into()],
        ..routine("a", "t", "claude", "0 * * * *", &[], &[], true)
    };
    let other = Routine {
        tags: vec!["prod".into()],
        ..routine("b", "t", "claude", "0 * * * *", &[], &[], true)
    };
    let none = routine("c", "t", "claude", "0 * * * *", &[], &[], true);
    assert!(f.matches(&hit, now(), window()));
    assert!(!f.matches(&other, now(), window()));
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
fn query_matches_tag() {
    let f = RoutineFilter {
        query: "nightly".into(),
        ..Default::default()
    };
    let hit = Routine {
        tags: vec!["nightly".into()],
        ..routine("a", "t", "claude", "0 * * * *", &[], &[], true)
    };
    let miss = Routine {
        tags: vec!["prod".into()],
        ..routine("b", "t", "claude", "0 * * * *", &[], &[], true)
    };
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
fn distinct_repositories_returns_sorted_unique_repositories() {
    let routines = vec![
        routine(
            "a",
            "t",
            "claude",
            "0 * * * *",
            &[],
            &["repo-b", "repo-a"],
            true,
        ),
        routine(
            "b",
            "t",
            "claude",
            "0 * * * *",
            &[],
            &["repo-a", "repo-c"],
            true,
        ),
    ];
    let repos = distinct_repositories(&routines);
    assert_eq!(repos, vec!["repo-a", "repo-b", "repo-c"]);
}

#[test]
fn distinct_tags_returns_sorted_unique_tags() {
    let routines = vec![
        Routine {
            tags: vec!["nightly".into(), "beta".into()],
            ..routine("a", "t", "claude", "0 * * * *", &[], &[], true)
        },
        Routine {
            tags: vec!["beta".into(), "prod".into()],
            ..routine("b", "t", "claude", "0 * * * *", &[], &[], true)
        },
    ];
    let tags = distinct_tags(&routines);
    assert_eq!(tags, vec!["beta", "nightly", "prod"]);
}
