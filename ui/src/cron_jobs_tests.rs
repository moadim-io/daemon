//! Host-side unit tests for the cron-jobs faceted filter: the `StatusFacet` /
//! `MachineFacet` codecs and the pure `JobFilter` matching + list helpers that
//! back the search box, status/machine facets, and live result count. All
//! deterministic given a fixed `now`; no DOM/wasm dependency (mirrors the
//! `schedule.rs` / `overview.rs` test conventions).

use super::*;
use chrono::{Local, TimeZone};

/// A fixed reference instant 30s past the top of the hour, so a top-of-hour
/// schedule's next fire (13:00) lands ~59.5m out — inside a 1h "due soon"
/// window but not a 5m one.
fn now() -> DateTime<Local> {
    Local.with_ymd_and_hms(2026, 6, 22, 12, 0, 30).unwrap()
}

fn window() -> Duration {
    Duration::seconds(DUE_SOON_WINDOW_SECS)
}

/// Build a job with the fields the filter reads; the rest are inert.
fn job(id: &str, handler: &str, schedule: &str, machines: &[&str], enabled: bool) -> CronJob {
    CronJob {
        id: id.into(),
        schedule: schedule.into(),
        handler: handler.into(),
        metadata: serde_json::json!({}),
        machines: machines.iter().map(|m| (*m).to_string()).collect(),
        enabled,
        created_at: 0,
        updated_at: 0,
        last_manual_trigger_at: None,
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

// ── is_active ─────────────────────────────────────────────────────────────────

#[test]
fn default_filter_is_inactive() {
    assert!(!JobFilter::default().is_active());
}

#[test]
fn is_active_detects_each_facet() {
    let q = JobFilter {
        query: "  x ".into(),
        ..Default::default()
    };
    assert!(q.is_active());
    // Whitespace-only query is not active.
    let blank = JobFilter {
        query: "   ".into(),
        ..Default::default()
    };
    assert!(!blank.is_active());

    let s = JobFilter {
        status: StatusFacet::Enabled,
        ..Default::default()
    };
    assert!(s.is_active());

    let m = JobFilter {
        machine: MachineFacet::Unassigned,
        ..Default::default()
    };
    assert!(m.is_active());
}

// ── Status facet matching ─────────────────────────────────────────────────────

#[test]
fn status_all_matches_regardless_of_enabled() {
    let f = JobFilter::default();
    assert!(f.matches(&job("a", "h", "0 * * * *", &[], true), now(), window()));
    assert!(f.matches(&job("b", "h", "0 * * * *", &[], false), now(), window()));
}

#[test]
fn status_enabled_and_disabled_partition() {
    let on = job("a", "h", "0 * * * *", &[], true);
    let off = job("b", "h", "0 * * * *", &[], false);
    let enabled = JobFilter {
        status: StatusFacet::Enabled,
        ..Default::default()
    };
    let disabled = JobFilter {
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
    let f = JobFilter {
        status: StatusFacet::DueSoon,
        ..Default::default()
    };
    // Enabled, fires at the next top of hour (~59.5m) → inside the 1h window.
    let soon = job("a", "h", "0 * * * *", &[], true);
    assert!(f.matches(&soon, now(), window()));
    // Same schedule but disabled → never "due".
    let soon_off = job("b", "h", "0 * * * *", &[], false);
    assert!(!f.matches(&soon_off, now(), window()));
    // Enabled but far away (annually, Jan 1) → outside the window.
    let far = job("c", "h", "0 0 1 1 *", &[], true);
    assert!(!f.matches(&far, now(), window()));
}

// ── Machine facet matching ────────────────────────────────────────────────────

#[test]
fn machine_any_matches_all() {
    let f = JobFilter::default();
    assert!(f.matches(&job("a", "h", "0 * * * *", &["m1"], true), now(), window()));
    assert!(f.matches(&job("b", "h", "0 * * * *", &[], true), now(), window()));
}

#[test]
fn machine_specific_requires_membership() {
    let f = JobFilter {
        machine: MachineFacet::Machine("m1".into()),
        ..Default::default()
    };
    assert!(f.matches(
        &job("a", "h", "0 * * * *", &["m1", "m2"], true),
        now(),
        window()
    ));
    assert!(!f.matches(&job("b", "h", "0 * * * *", &["m2"], true), now(), window()));
    assert!(!f.matches(&job("c", "h", "0 * * * *", &[], true), now(), window()));
}

#[test]
fn machine_unassigned_matches_only_empty() {
    let f = JobFilter {
        machine: MachineFacet::Unassigned,
        ..Default::default()
    };
    assert!(f.matches(&job("a", "h", "0 * * * *", &[], true), now(), window()));
    assert!(!f.matches(&job("b", "h", "0 * * * *", &["m1"], true), now(), window()));
}

// ── Free-text matching ────────────────────────────────────────────────────────

#[test]
fn query_matches_across_id_handler_and_schedule() {
    let j = job("nightly-backup", "run-backup", "0 3 * * *", &[], true);
    let by_id = JobFilter {
        query: "nightly".into(),
        ..Default::default()
    };
    let by_handler = JobFilter {
        query: "BACKUP".into(),
        ..Default::default()
    };
    let by_schedule = JobFilter {
        query: "0 3".into(),
        ..Default::default()
    };
    assert!(by_id.matches(&j, now(), window()));
    assert!(by_handler.matches(&j, now(), window())); // case-insensitive
    assert!(by_schedule.matches(&j, now(), window()));
}

#[test]
fn query_matches_description_and_metadata() {
    let mut j = job("j1", "h", "0 3 * * *", &[], true);
    j.schedule_description = Some("Every day at 03:00".into());
    j.metadata = serde_json::json!({ "recipient": "ops-team@example.com" });
    let by_desc = JobFilter {
        query: "every day".into(),
        ..Default::default()
    };
    let by_meta = JobFilter {
        query: "ops-team".into(),
        ..Default::default()
    };
    assert!(by_desc.matches(&j, now(), window()));
    assert!(by_meta.matches(&j, now(), window()));
}

#[test]
fn query_is_trimmed_and_non_match_is_excluded() {
    let j = job("alpha", "beta", "0 3 * * *", &[], true);
    let trimmed = JobFilter {
        query: "  alpha  ".into(),
        ..Default::default()
    };
    let miss = JobFilter {
        query: "zeta".into(),
        ..Default::default()
    };
    assert!(trimmed.matches(&j, now(), window()));
    assert!(!miss.matches(&j, now(), window()));
}

#[test]
fn facets_and_together() {
    // Enabled AND on m1 AND query "back".
    let f = JobFilter {
        query: "back".into(),
        status: StatusFacet::Enabled,
        machine: MachineFacet::Machine("m1".into()),
    };
    let hit = job("backup", "h", "0 * * * *", &["m1"], true);
    let wrong_machine = job("backup", "h", "0 * * * *", &["m2"], true);
    let disabled = job("backup", "h", "0 * * * *", &["m1"], false);
    let wrong_text = job("cleanup", "h", "0 * * * *", &["m1"], true);
    assert!(f.matches(&hit, now(), window()));
    assert!(!f.matches(&wrong_machine, now(), window()));
    assert!(!f.matches(&disabled, now(), window()));
    assert!(!f.matches(&wrong_text, now(), window()));
}

// ── List helpers ──────────────────────────────────────────────────────────────

#[test]
fn filter_jobs_preserves_order_and_narrows() {
    let jobs = vec![
        job("a", "keep", "0 * * * *", &[], true),
        job("b", "drop", "0 * * * *", &[], false),
        job("c", "keep", "0 * * * *", &[], true),
    ];
    let f = JobFilter {
        status: StatusFacet::Enabled,
        ..Default::default()
    };
    let out = filter_jobs(&jobs, &f, now(), window());
    let ids: Vec<&str> = out.iter().map(|j| j.id.as_str()).collect();
    assert_eq!(ids, vec!["a", "c"]);
}

#[test]
fn filter_jobs_unfiltered_returns_all() {
    let jobs = vec![
        job("a", "h", "0 * * * *", &[], true),
        job("b", "h", "0 * * * *", &[], false),
    ];
    let out = filter_jobs(&jobs, &JobFilter::default(), now(), window());
    assert_eq!(out.len(), 2);
}

#[test]
fn distinct_machines_are_sorted_and_deduped() {
    let jobs = vec![
        job("a", "h", "0 * * * *", &["m2", "m1"], true),
        job("b", "h", "0 * * * *", &["m1", "m3"], true),
        job("c", "h", "0 * * * *", &[], true),
    ];
    assert_eq!(distinct_machines(&jobs), vec!["m1", "m2", "m3"]);
}

#[test]
fn unassigned_count_tallies_dormant_jobs() {
    let jobs = vec![
        job("a", "h", "0 * * * *", &["m1"], true),
        job("b", "h", "0 * * * *", &[], true),
        job("c", "h", "0 * * * *", &[], false),
    ];
    assert_eq!(unassigned_count(&jobs), 2);
}
