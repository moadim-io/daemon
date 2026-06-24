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

// ── Sort ──────────────────────────────────────────────────────────────────────

#[test]
fn sort_dir_flip_cycles() {
    assert_eq!(SortDir::Asc.flip(), SortDir::Desc);
    assert_eq!(SortDir::Desc.flip(), SortDir::Asc);
}

#[test]
fn sort_dir_default_is_asc() {
    assert_eq!(SortDir::default(), SortDir::Asc);
}

#[test]
fn sort_jobs_none_preserves_input_order() {
    let jobs = vec![
        job("c", "h", "0 * * * *", &[], true),
        job("a", "h", "0 * * * *", &[], true),
        job("b", "h", "0 * * * *", &[], true),
    ];
    let out = sort_jobs(jobs.clone(), None, SortDir::Asc, now());
    assert_eq!(out, jobs);
}

#[test]
fn sort_jobs_by_id_ascending() {
    let jobs = vec![
        job("c-job", "h", "0 * * * *", &[], true),
        job("a-job", "h", "0 * * * *", &[], true),
        job("b-job", "h", "0 * * * *", &[], true),
    ];
    let out = sort_jobs(jobs, Some(SortCol::Id), SortDir::Asc, now());
    let ids: Vec<&str> = out.iter().map(|j| j.id.as_str()).collect();
    assert_eq!(ids, vec!["a-job", "b-job", "c-job"]);
}

#[test]
fn sort_jobs_by_id_descending() {
    let jobs = vec![
        job("a-job", "h", "0 * * * *", &[], true),
        job("b-job", "h", "0 * * * *", &[], true),
        job("c-job", "h", "0 * * * *", &[], true),
    ];
    let out = sort_jobs(jobs, Some(SortCol::Id), SortDir::Desc, now());
    let ids: Vec<&str> = out.iter().map(|j| j.id.as_str()).collect();
    assert_eq!(ids, vec!["c-job", "b-job", "a-job"]);
}

#[test]
fn sort_jobs_by_handler_ascending() {
    let jobs = vec![
        job("j1", "zap", "0 * * * *", &[], true),
        job("j2", "alpha", "0 * * * *", &[], true),
        job("j3", "beta", "0 * * * *", &[], true),
    ];
    let out = sort_jobs(jobs, Some(SortCol::Handler), SortDir::Asc, now());
    let handlers: Vec<&str> = out.iter().map(|j| j.handler.as_str()).collect();
    assert_eq!(handlers, vec!["alpha", "beta", "zap"]);
}

#[test]
fn sort_jobs_by_handler_descending() {
    let jobs = vec![
        job("j1", "alpha", "0 * * * *", &[], true),
        job("j2", "zap", "0 * * * *", &[], true),
        job("j3", "beta", "0 * * * *", &[], true),
    ];
    let out = sort_jobs(jobs, Some(SortCol::Handler), SortDir::Desc, now());
    let handlers: Vec<&str> = out.iter().map(|j| j.handler.as_str()).collect();
    assert_eq!(handlers, vec!["zap", "beta", "alpha"]);
}

#[test]
fn sort_jobs_by_enabled_asc_disabled_first() {
    let jobs = vec![
        job("j1", "h", "0 * * * *", &[], true),
        job("j2", "h", "0 * * * *", &[], false),
        job("j3", "h", "0 * * * *", &[], true),
    ];
    let out = sort_jobs(jobs, Some(SortCol::Enabled), SortDir::Asc, now());
    // false < true → disabled rows first in Asc
    assert!(!out[0].enabled);
    assert!(out[1].enabled);
    assert!(out[2].enabled);
}

#[test]
fn sort_jobs_by_enabled_desc_enabled_first() {
    let jobs = vec![
        job("j1", "h", "0 * * * *", &[], false),
        job("j2", "h", "0 * * * *", &[], true),
        job("j3", "h", "0 * * * *", &[], false),
    ];
    let out = sort_jobs(jobs, Some(SortCol::Enabled), SortDir::Desc, now());
    assert!(out[0].enabled);
    assert!(!out[1].enabled);
    assert!(!out[2].enabled);
}

#[test]
fn sort_jobs_by_updated_ascending() {
    let mut j1 = job("j1", "h", "0 * * * *", &[], true);
    j1.updated_at = 300;
    let mut j2 = job("j2", "h", "0 * * * *", &[], true);
    j2.updated_at = 100;
    let mut j3 = job("j3", "h", "0 * * * *", &[], true);
    j3.updated_at = 200;
    let out = sort_jobs(
        vec![j1, j2, j3],
        Some(SortCol::Updated),
        SortDir::Asc,
        now(),
    );
    let ids: Vec<&str> = out.iter().map(|j| j.id.as_str()).collect();
    assert_eq!(ids, vec!["j2", "j3", "j1"]);
}

#[test]
fn sort_jobs_by_updated_descending() {
    let mut j1 = job("j1", "h", "0 * * * *", &[], true);
    j1.updated_at = 100;
    let mut j2 = job("j2", "h", "0 * * * *", &[], true);
    j2.updated_at = 300;
    let mut j3 = job("j3", "h", "0 * * * *", &[], true);
    j3.updated_at = 200;
    let out = sort_jobs(
        vec![j1, j2, j3],
        Some(SortCol::Updated),
        SortDir::Desc,
        now(),
    );
    let ids: Vec<&str> = out.iter().map(|j| j.id.as_str()).collect();
    assert_eq!(ids, vec!["j2", "j3", "j1"]);
}

#[test]
fn sort_jobs_by_next_run_enabled_before_disabled() {
    // Disabled jobs have no next-run time → sort to end in Asc.
    let enabled = job("j1", "h", "0 * * * *", &[], true);
    let disabled = job("j2", "h", "0 * * * *", &[], false);
    let out = sort_jobs(
        vec![disabled.clone(), enabled.clone()],
        Some(SortCol::NextRun),
        SortDir::Asc,
        now(),
    );
    assert_eq!(out[0].id, enabled.id);
    assert_eq!(out[1].id, disabled.id);
}

#[test]
fn sort_jobs_by_next_run_desc_disabled_before_enabled() {
    // Desc reverses: disabled (None) sorts before the enabled job.
    let enabled = job("j1", "h", "0 * * * *", &[], true);
    let disabled = job("j2", "h", "0 * * * *", &[], false);
    let out = sort_jobs(
        vec![enabled.clone(), disabled.clone()],
        Some(SortCol::NextRun),
        SortDir::Desc,
        now(),
    );
    assert_eq!(out[0].id, disabled.id);
    assert_eq!(out[1].id, enabled.id);
}

#[test]
fn sort_jobs_stable_tiebreak_by_id() {
    // Equal handlers → tiebreak by id ascending.
    let jobs = vec![
        job("beta", "same-handler", "0 * * * *", &[], true),
        job("alpha", "same-handler", "0 * * * *", &[], true),
    ];
    let out = sort_jobs(jobs, Some(SortCol::Handler), SortDir::Asc, now());
    assert_eq!(out[0].id, "alpha");
    assert_eq!(out[1].id, "beta");
}
