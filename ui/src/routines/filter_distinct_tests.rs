use super::filter_tests::routine;
use super::*;

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
