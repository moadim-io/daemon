#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use super::*;

/// Split out of `mod_agents_tests.rs` to stay under the repo's 500-line-per-file cap: every test
/// here exercises `svc_list`/`svc_get`'s reload-from-disk-on-every-GET behavior specifically.
fn make_routine(id: &str) -> Routine {
    Routine {
        model: None,
        id: id.to_string(),
        schedule: "@daily".to_string(),
        title: "My Routine".to_string(),
        agent: "claude".to_string(),
        prompt: "do the thing".to_string(),
        goal: None,
        repositories: vec![Repository {
            repository: "https://github.com/octocat/Hello-World".to_string(),
            branch: Some("master".to_string()),
        }],
        machines: vec![crate::machine::current_machine()],
        enabled: true,
        source: "managed".to_string(),
        created_at: 0,
        updated_at: 0,
        last_manual_trigger_at: None,
        last_scheduled_trigger_at: None,
        snoozed_until: None,
        skip_runs: None,
        power_saving: false,
        tags: vec![],
        ttl_secs: None,
        max_runtime_secs: None,
    }
}

/// A unique, freshly-created scratch directory under the system temp dir, used as the on-disk
/// source the GET read path re-scans. `svc_list`/`svc_get` reload the store from this dir before
/// serving, so tests persist their routines here to exercise the real reload in isolation.
fn scratch_routines_dir() -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("moadim-rt-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Write `routine` to `{base}/{routine.id}/routine.toml` so the directory-aware reload in
/// `svc_list`/`svc_get` loads it back, keyed by the `id` inside the file.
///
/// The scan keys routines by the `id` field in `routine.toml` (the directory name is only the scan
/// entry), so using the id as the dir name keeps fixtures with identical titles from colliding the
/// way the slug-based on-disk layout would. Rooted at an arbitrary `base` rather than the global
/// routines dir, keeping the test self-contained and parallel with no shared global state.
fn write_routine_to(base: &std::path::Path, routine: &Routine) {
    use std::fmt::Write as _;
    let dir = base.join(&routine.id);
    std::fs::create_dir_all(&dir).unwrap();
    let mut toml = format!(
        "id = \"{}\"\nschedule = \"{}\"\ntitle = \"{}\"\nagent = \"{}\"\nprompt = \"{}\"\nenabled = {}\ncreated_at = {}\nupdated_at = {}\nmachines = {:?}\ntags = {:?}\n",
        routine.id,
        routine.schedule,
        routine.title,
        routine.agent,
        routine.prompt,
        routine.enabled,
        routine.created_at,
        routine.updated_at,
        routine.machines,
        routine.tags,
    );
    for repo in &routine.repositories {
        toml.push_str("\n[[repositories]]\n");
        let _ = writeln!(toml, "repository = \"{}\"", repo.repository);
        if let Some(branch) = &repo.branch {
            let _ = writeln!(toml, "branch = \"{branch}\"");
        }
    }
    std::fs::write(dir.join("routine.toml"), toml).unwrap();
}

#[test]
fn svc_get_not_found() {
    assert!(svc_get(&new_store(), &scratch_routines_dir(), "missing").is_err());
}

#[test]
fn svc_list_empty() {
    assert!(svc_list(
        &new_store(),
        &scratch_routines_dir(),
        &RoutineListQuery::default()
    )
    .is_empty());
}

#[test]
fn svc_list_sorted_by_created_at() {
    let dir = scratch_routines_dir();
    let mut early = make_routine("early");
    early.created_at = 10;
    let mut late = make_routine("late");
    late.created_at = 20;
    write_routine_to(&dir, &late);
    write_routine_to(&dir, &early);
    // The store starts empty; `svc_list` reloads both routines from disk and sorts by created_at.
    let list = svc_list(&new_store(), &dir, &RoutineListQuery::default());
    assert_eq!(list[0].routine.id, "early");
    assert_eq!(list[1].routine.id, "late");
}

#[test]
fn svc_list_descending_order() {
    let dir = scratch_routines_dir();
    let mut early = make_routine("early");
    early.created_at = 10;
    let mut late = make_routine("late");
    late.created_at = 20;
    write_routine_to(&dir, &early);
    write_routine_to(&dir, &late);
    let query = RoutineListQuery {
        order: SortOrder::Desc,
        ..Default::default()
    };
    let list = svc_list(&new_store(), &dir, &query);
    assert_eq!(list[0].routine.id, "late");
    assert_eq!(list[1].routine.id, "early");
}

#[test]
fn svc_list_filters_by_repository_substring() {
    let dir = scratch_routines_dir();
    let mut alpha = make_routine("alpha");
    alpha.repositories = vec![Repository {
        repository: "https://github.com/octocat/Alpha".to_string(),
        branch: None,
    }];
    let mut beta = make_routine("beta");
    beta.repositories = vec![Repository {
        repository: "https://github.com/octocat/Beta".to_string(),
        branch: None,
    }];
    write_routine_to(&dir, &alpha);
    write_routine_to(&dir, &beta);
    let query = RoutineListQuery {
        // Case-insensitive substring match.
        repository: Some("alpha".to_string()),
        ..Default::default()
    };
    let list = svc_list(&new_store(), &dir, &query);
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].routine.id, "alpha");
}

#[test]
fn svc_list_sorts_by_repository_no_repo_last() {
    let dir = scratch_routines_dir();
    let mut zeta = make_routine("zeta");
    zeta.repositories = vec![Repository {
        repository: "https://github.com/octocat/Zeta".to_string(),
        branch: None,
    }];
    let mut apple = make_routine("apple");
    apple.repositories = vec![Repository {
        repository: "https://github.com/octocat/Apple".to_string(),
        branch: None,
    }];
    let mut none = make_routine("none");
    none.repositories = vec![];
    write_routine_to(&dir, &zeta);
    write_routine_to(&dir, &apple);
    write_routine_to(&dir, &none);
    let query = RoutineListQuery {
        sort: RoutineSort::Repository,
        ..Default::default()
    };
    let list = svc_list(&new_store(), &dir, &query);
    assert_eq!(list[0].routine.id, "apple");
    assert_eq!(list[1].routine.id, "zeta");
    // Routines with no repository sort last.
    assert_eq!(list[2].routine.id, "none");
}

#[test]
fn svc_get_reflects_routine_written_after_store_built() {
    // A routine written to disk *after* the (empty) store was built becomes visible on the next get
    // without rebuilding the store — the core "load the machines in every get" fix.
    let dir = scratch_routines_dir();
    let store = new_store();
    assert!(svc_get(&store, &dir, "appears").is_err());
    write_routine_to(&dir, &make_routine("appears"));
    assert_eq!(
        svc_get(&store, &dir, "appears").unwrap().routine.id,
        "appears"
    );
}

#[test]
fn svc_get_reflects_routine_removed_on_disk() {
    // A routine removed on disk disappears from the next get without a restart.
    let dir = scratch_routines_dir();
    write_routine_to(&dir, &make_routine("gone"));
    let store = new_store();
    assert!(svc_get(&store, &dir, "gone").is_ok());
    std::fs::remove_dir_all(dir.join("gone")).unwrap();
    assert!(svc_get(&store, &dir, "gone").is_err());
}

#[test]
fn svc_get_reflects_machines_edit_on_disk() {
    // Editing a routine's `machines` list on disk (e.g. via a `git pull`) is reflected on the next
    // get without a restart — the exact field the user reported as stale.
    let dir = scratch_routines_dir();
    let mut routine = make_routine("machines-edit");
    routine.machines = vec!["host-a".to_string()];
    write_routine_to(&dir, &routine);
    let store = new_store();
    assert_eq!(
        svc_get(&store, &dir, "machines-edit")
            .unwrap()
            .routine
            .machines,
        vec!["host-a".to_string()]
    );
    routine.machines = vec!["host-a".to_string(), "host-b".to_string()];
    write_routine_to(&dir, &routine);
    assert_eq!(
        svc_get(&store, &dir, "machines-edit")
            .unwrap()
            .routine
            .machines,
        vec!["host-a".to_string(), "host-b".to_string()]
    );
}

#[test]
fn reload_preserves_last_scheduled_trigger_at_sidecar() {
    // The reload goes through the same load path that reads the gitignored `scheduled.log`
    // append-only log, so the scheduler-written `last_scheduled_trigger_at` survives a reload
    // rather than being clobbered.
    let dir = scratch_routines_dir();
    write_routine_to(&dir, &make_routine("sched"));
    // The launch command appends to this log at each scheduled firing; the daemon only reads it.
    std::fs::write(dir.join("sched").join("scheduled.log"), "1717000000\n").unwrap();
    let store = new_store();
    let resp = svc_get(&store, &dir, "sched").unwrap();
    assert_eq!(resp.routine.last_scheduled_trigger_at, Some(1_717_000_000));
}
