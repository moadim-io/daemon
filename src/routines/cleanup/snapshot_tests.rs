#![allow(clippy::missing_docs_in_private_items)]

use super::super::super::command::slugify;
use super::super::super::model::{new_store, Routine};
use super::super::ttl::MAX_TTL_SECS;
use super::*;

fn routine_with(title: &str, schedule: &str, ttl_secs: Option<u64>) -> Routine {
    Routine {
        id: "id".into(),
        schedule: schedule.into(),
        title: title.into(),
        agent: "claude".into(),
        prompt: "p".into(),
        repositories: vec![],
        enabled: true,
        source: "managed".into(),
        created_at: 0,
        updated_at: 0,
        last_triggered_at: None,
        ttl_secs,
    }
}

#[test]
fn snapshot_ttls_maps_slug_to_effective_ttl() {
    let store = new_store();
    store.lock().unwrap().insert(
        "id".into(),
        routine_with("My Routine", "*/10 * * * *", None),
    );

    let snapshot = snapshot_ttls(&store);
    // Title "My Routine" slugifies; the sub-hour interval yields a 600s TTL.
    let slug = slugify("My Routine");
    assert_eq!(snapshot.get(&slug).copied(), Some(10 * 60));
}

#[test]
fn snapshot_ttls_empty_store_is_empty() {
    assert!(snapshot_ttls(&new_store()).is_empty());
}

#[test]
fn ttl_for_returns_snapshot_value_when_present() {
    let mut snapshot = HashMap::new();
    snapshot.insert("known".to_string(), 42u64);
    assert_eq!(ttl_for(&snapshot, "known"), 42);
}

#[test]
fn ttl_for_falls_back_to_max_for_orphan_slug() {
    let snapshot: HashMap<String, u64> = HashMap::new();
    assert_eq!(ttl_for(&snapshot, "orphan"), MAX_TTL_SECS);
}
