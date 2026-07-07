#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use super::*;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Point `MOADIM_HOME_OVERRIDE` at a fresh, empty temp home for the duration of a test, removing the
/// env var and the temp dir on drop. This keeps `svc_create`/`svc_update`/`write_routine` and the
/// other disk-touching paths off the developer's real `~/.moadim`, so a panicking assertion can never
/// leak test routines into the real home. Tests in this crate run single-threaded
/// (`RUST_TEST_THREADS=1`), so the global env mutation is safe.
///
/// `svc_list`/`svc_get` reload the store from disk before serving (see [`crate::routine_storage`]),
/// so every test here needs a `TempHome` regardless of whether it also builds an in-memory store —
/// otherwise the reload would either wipe the in-memory fixture or, worse, read the developer's real
/// `~/.config/moadim/routines`.
struct TempHome(std::path::PathBuf);

impl TempHome {
    fn set() -> Self {
        let dir = std::env::temp_dir().join(format!("moadim-svctest-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("create temp home");
        // SAFETY: single-threaded test execution.
        unsafe {
            std::env::set_var("MOADIM_HOME_OVERRIDE", &dir);
        }
        Self(dir)
    }
}

impl Drop for TempHome {
    fn drop(&mut self) {
        // SAFETY: single-threaded test execution.
        unsafe {
            std::env::remove_var("MOADIM_HOME_OVERRIDE");
        }
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Build a routine with overridable identity, title, timestamps, and repository URL.
fn make_routine(id: &str, title: &str, created_at: u64, updated_at: u64) -> Routine {
    Routine {
        model: None,
        id: id.to_string(),
        schedule: "@daily".to_string(),
        title: title.to_string(),
        agent: "claude".to_string(),
        prompt: "do the thing".to_string(),
        goal: None,
        repositories: vec![],
        machines: vec![crate::machine::current_machine()],
        enabled: true,
        source: "managed".to_string(),
        created_at,
        updated_at,
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

/// Wrap a list of routines into a populated [`RoutineStore`], and persist each one to disk (under
/// the active `TempHome`) so `svc_list`'s reload-from-disk doesn't wipe the fixture the test just
/// built — disk is the source of truth the reload re-scans on every call.
fn store_with(routines: Vec<Routine>) -> RoutineStore {
    let mut map = HashMap::new();
    for routine in routines {
        write_routine(&routine).expect("write_routine");
        map.insert(routine.id.clone(), routine);
    }
    Arc::new(Mutex::new(map))
}

#[test]
fn svc_list_sorts_by_updated_at() {
    let _home = TempHome::set();
    // Covers the `RoutineSort::Updated` arm: sort by `updated_at` ascending.
    let store = store_with(vec![
        make_routine("late", "Zeta", 100, 300),
        make_routine("early", "Alpha", 100, 100),
        make_routine("mid", "Mid", 100, 200),
    ]);
    let query = RoutineListQuery {
        sort: RoutineSort::Updated,
        ..Default::default()
    };
    let list = svc_list(&store, &crate::paths::routines_dir(), &query);
    assert_eq!(list[0].routine.id, "early");
    assert_eq!(list[1].routine.id, "mid");
    assert_eq!(list[2].routine.id, "late");
}

#[test]
fn svc_list_sorts_by_title_case_insensitively() {
    let _home = TempHome::set();
    // Covers the `RoutineSort::Title` arm: sort by lowercased title.
    let store = store_with(vec![
        make_routine("banana", "banana", 0, 0),
        make_routine("apple", "Apple", 0, 0),
        make_routine("cherry", "CHERRY", 0, 0),
    ]);
    let query = RoutineListQuery {
        sort: RoutineSort::Title,
        ..Default::default()
    };
    let list = svc_list(&store, &crate::paths::routines_dir(), &query);
    assert_eq!(list[0].routine.id, "apple");
    assert_eq!(list[1].routine.id, "banana");
    assert_eq!(list[2].routine.id, "cherry");
}

#[test]
fn svc_list_breaks_ties_on_id_deterministically() {
    // Routines come off a `HashMap` (unspecified iteration order), so equal
    // sort keys must be broken on the stable routine id for the listing to be
    // deterministic. Three routines share a `created_at`; ascending lists them
    // by id (A→Z) and descending reverses that, never an arbitrary order.
    let _home = TempHome::set();
    let tied = || {
        store_with(vec![
            make_routine("charlie", "C", 50, 0),
            make_routine("alpha", "A", 50, 0),
            make_routine("bravo", "B", 50, 0),
        ])
    };

    let asc = svc_list(&tied(), &crate::paths::routines_dir(), &RoutineListQuery::default());
    assert_eq!(
        asc.iter().map(|resp| &resp.routine.id).collect::<Vec<_>>(),
        ["alpha", "bravo", "charlie"],
    );

    let desc = svc_list(
        &tied(),
        &crate::paths::routines_dir(),
        &RoutineListQuery {
            order: SortOrder::Desc,
            ..Default::default()
        },
    );
    assert_eq!(
        desc.iter().map(|resp| &resp.routine.id).collect::<Vec<_>>(),
        ["charlie", "bravo", "alpha"],
    );
}

#[test]
fn svc_list_omits_prompt_by_default() {
    let _home = TempHome::set();
    // Default query leaves the prompt blank, and `skip_serializing_if` drops the field entirely.
    let store = store_with(vec![make_routine("a", "Alpha", 0, 0)]);
    let list = svc_list(&store, &crate::paths::routines_dir(), &RoutineListQuery::default());
    assert_eq!(list.len(), 1);
    assert!(list[0].routine.prompt.is_empty());
    let json = serde_json::to_value(&list[0]).unwrap();
    assert!(
        json.get("prompt").is_none(),
        "prompt should be absent from the serialized listing, got {json}"
    );
}

#[test]
fn svc_list_includes_prompt_when_requested() {
    let _home = TempHome::set();
    let store = store_with(vec![make_routine("a", "Alpha", 0, 0)]);
    let query = RoutineListQuery {
        include_prompts: Some(true),
        ..Default::default()
    };
    let list = svc_list(&store, &crate::paths::routines_dir(), &query);
    assert_eq!(list[0].routine.prompt, "do the thing");
    let json = serde_json::to_value(&list[0]).unwrap();
    assert_eq!(
        json.get("prompt").and_then(|value| value.as_str()),
        Some("do the thing")
    );
}
