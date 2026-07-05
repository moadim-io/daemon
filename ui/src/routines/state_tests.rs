use super::super::model::Repository;
use super::*;
use chrono::TimeZone;
use std::rc::Rc;
use yew::Reducible;

/// Build a routine with the fields the filter/state helpers read; the rest are inert.
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

// ── Bulk selection reducer actions ────────────────────────────────────────────

fn state_with_routines(ids: &[&str]) -> Rc<RState> {
    let routines = ids
        .iter()
        .map(|id| routine(id, id, "claude", "0 * * * *", &["m1"], &[], true))
        .collect();
    Rc::new(RState {
        routines,
        loading: false,
        ..RState::default()
    })
}

#[test]
fn select_routine_adds_id_to_selection() {
    let s = state_with_routines(&["a", "b"]);
    let s = s.reduce(RAction::SelectRoutine("a".into()));
    assert!(s.selected.contains("a"));
    assert!(!s.selected.contains("b"));
}

#[test]
fn select_routine_toggles_already_selected_id_out() {
    let s = state_with_routines(&["a"]);
    let s = s.reduce(RAction::SelectRoutine("a".into()));
    assert!(s.selected.contains("a"));
    let s = s.reduce(RAction::SelectRoutine("a".into()));
    assert!(!s.selected.contains("a"));
}

#[test]
fn select_all_replaces_selection() {
    let s = state_with_routines(&["a", "b", "c"]);
    let s = s.reduce(RAction::SelectAll(vec!["a".into(), "c".into()]));
    assert!(s.selected.contains("a"));
    assert!(!s.selected.contains("b"));
    assert!(s.selected.contains("c"));
}

#[test]
fn clear_selection_empties_all() {
    let s = state_with_routines(&["a", "b"]);
    let s = s.reduce(RAction::SelectAll(vec!["a".into(), "b".into()]));
    assert_eq!(s.selected.len(), 2);
    let s = s.reduce(RAction::ClearSelection);
    assert!(s.selected.is_empty());
}

#[test]
fn open_confirm_bulk_delete_sets_modal_with_count() {
    let s = state_with_routines(&["a", "b"]);
    let s = s.reduce(RAction::SelectAll(vec!["a".into(), "b".into()]));
    let s = s.reduce(RAction::OpenConfirmBulkDelete);
    assert_eq!(s.modal, RModal::ConfirmBulkDelete { count: 2 });
}

#[test]
fn remove_many_removes_routines_and_clears_from_selection() {
    let s = state_with_routines(&["a", "b", "c"]);
    let s = s.reduce(RAction::SelectAll(vec!["a".into(), "b".into(), "c".into()]));
    let s = s.reduce(RAction::RemoveMany(vec!["a".into(), "c".into()]));
    assert_eq!(s.routines.len(), 1);
    assert_eq!(s.routines[0].id, "b");
    assert!(!s.selected.contains("a"));
    assert!(s.selected.contains("b"));
    assert!(!s.selected.contains("c"));
}

#[test]
fn loaded_drops_stale_selections() {
    let s = state_with_routines(&["a", "b"]);
    let s = s.reduce(RAction::SelectAll(vec!["a".into(), "b".into()]));
    // Reload with only "a" — "b" should be dropped from selection.
    let new_routines = vec![routine("a", "a", "claude", "0 * * * *", &["m1"], &[], true)];
    let s = s.reduce(RAction::Loaded(new_routines));
    assert!(s.selected.contains("a"));
    assert!(!s.selected.contains("b"));
}

#[test]
fn remove_also_clears_from_selection() {
    let s = state_with_routines(&["a", "b"]);
    let s = s.reduce(RAction::SelectAll(vec!["a".into(), "b".into()]));
    let s = s.reduce(RAction::Remove("a".into()));
    assert!(!s.selected.contains("a"));
    assert!(s.selected.contains("b"));
    assert_eq!(s.routines.len(), 1);
}

// ── sort_routines ─────────────────────────────────────────────────────────────

fn routine_sort(id: &str, title: &str, agent: &str, enabled: bool, updated_at: u64) -> Routine {
    let mut r = routine(id, title, agent, "0 * * * *", &[], &[], enabled);
    r.updated_at = updated_at;
    r
}

#[test]
fn rdir_flip_toggles_direction() {
    assert_eq!(RDir::Asc.flip(), RDir::Desc);
    assert_eq!(RDir::Desc.flip(), RDir::Asc);
}

#[test]
fn sort_routines_none_col_preserves_insertion_order() {
    let rs = vec![
        routine_sort("z", "Zebra", "claude", true, 10),
        routine_sort("a", "Alpha", "codex", true, 5),
    ];
    let sorted = sort_routines(rs.clone(), None, RDir::Asc, now());
    assert_eq!(sorted[0].id, "z");
    assert_eq!(sorted[1].id, "a");
}

#[test]
fn sort_routines_by_title_ascending() {
    let rs = vec![
        routine_sort("b", "Zebra", "claude", true, 10),
        routine_sort("a", "Alpha", "claude", true, 5),
        routine_sort("c", "Mango", "claude", true, 7),
    ];
    let sorted = sort_routines(rs, Some(RCol::Title), RDir::Asc, now());
    assert_eq!(sorted[0].title, "Alpha");
    assert_eq!(sorted[1].title, "Mango");
    assert_eq!(sorted[2].title, "Zebra");
}

#[test]
fn sort_routines_by_title_descending() {
    let rs = vec![
        routine_sort("b", "Zebra", "claude", true, 10),
        routine_sort("a", "Alpha", "claude", true, 5),
        routine_sort("c", "Mango", "claude", true, 7),
    ];
    let sorted = sort_routines(rs, Some(RCol::Title), RDir::Desc, now());
    assert_eq!(sorted[0].title, "Zebra");
    assert_eq!(sorted[1].title, "Mango");
    assert_eq!(sorted[2].title, "Alpha");
}

#[test]
fn sort_routines_by_agent_ascending() {
    let rs = vec![
        routine_sort("a", "T1", "codex", true, 1),
        routine_sort("b", "T2", "claude", true, 2),
    ];
    let sorted = sort_routines(rs, Some(RCol::Agent), RDir::Asc, now());
    assert_eq!(sorted[0].agent, "claude");
    assert_eq!(sorted[1].agent, "codex");
}

#[test]
fn sort_routines_by_updated_ascending() {
    let rs = vec![
        routine_sort("a", "T1", "claude", true, 100),
        routine_sort("b", "T2", "claude", true, 50),
        routine_sort("c", "T3", "claude", true, 75),
    ];
    let sorted = sort_routines(rs, Some(RCol::Updated), RDir::Asc, now());
    assert_eq!(sorted[0].id, "b");
    assert_eq!(sorted[1].id, "c");
    assert_eq!(sorted[2].id, "a");
}

#[test]
fn sort_routines_by_updated_descending() {
    let rs = vec![
        routine_sort("a", "T1", "claude", true, 100),
        routine_sort("b", "T2", "claude", true, 50),
        routine_sort("c", "T3", "claude", true, 75),
    ];
    let sorted = sort_routines(rs, Some(RCol::Updated), RDir::Desc, now());
    assert_eq!(sorted[0].id, "a");
    assert_eq!(sorted[1].id, "c");
    assert_eq!(sorted[2].id, "b");
}

#[test]
fn sort_routines_by_enabled_puts_disabled_first_ascending() {
    let rs = vec![
        routine_sort("a", "T1", "claude", true, 1),
        routine_sort("b", "T2", "claude", false, 2),
        routine_sort("c", "T3", "claude", true, 3),
    ];
    let sorted = sort_routines(rs, Some(RCol::Enabled), RDir::Asc, now());
    // false < true, so disabled goes first in Asc
    assert!(!sorted[0].enabled);
    assert!(sorted[1].enabled);
    assert!(sorted[2].enabled);
}

#[test]
fn sort_routines_by_next_run_puts_none_after_some() {
    // Disabled routines have no next run → sort to end.
    let rs = vec![
        routine_sort("dis", "Disabled", "claude", false, 1),
        routine_sort("hourly", "Hourly", "claude", true, 2),
    ];
    let sorted = sort_routines(rs, Some(RCol::NextRun), RDir::Asc, now());
    assert_eq!(sorted[0].id, "hourly");
    assert_eq!(sorted[1].id, "dis");
}

#[test]
fn sort_routines_title_is_case_insensitive() {
    let rs = vec![
        routine_sort("a", "zebra", "claude", true, 1),
        routine_sort("b", "ALPHA", "claude", true, 2),
    ];
    let sorted = sort_routines(rs, Some(RCol::Title), RDir::Asc, now());
    assert_eq!(sorted[0].title, "ALPHA");
    assert_eq!(sorted[1].title, "zebra");
}

// ── sort by RCol::Health ──────────────────────────────────────────────────────

fn routine_with_health(
    id: &str,
    enabled: bool,
    machines: &[&str],
    agent_registered: bool,
) -> Routine {
    Routine {
        agent_registered,
        ..routine(id, id, "claude", "0 * * * *", machines, &[], enabled)
    }
}

#[test]
fn sort_by_health_ascending_puts_most_broken_first() {
    let rs = vec![
        routine_with_health("healthy", true, &["m1"], true), // priority 4
        routine_with_health("dormant", true, &[], true),     // priority 0
        routine_with_health("disabled", false, &["m1"], false), // priority 3
    ];
    let sorted = sort_routines(rs, Some(RCol::Health), RDir::Asc, now());
    assert_eq!(sorted[0].id, "dormant");
    assert_eq!(sorted[1].id, "disabled");
    assert_eq!(sorted[2].id, "healthy");
}

#[test]
fn sort_by_health_descending_puts_healthy_first() {
    let rs = vec![
        routine_with_health("dormant", true, &[], true),
        routine_with_health("healthy", true, &["m1"], true),
    ];
    let sorted = sort_routines(rs, Some(RCol::Health), RDir::Desc, now());
    assert_eq!(sorted[0].id, "healthy");
    assert_eq!(sorted[1].id, "dormant");
}

// ── RGroupBy codec ────────────────────────────────────────────────────────────

#[test]
fn r_group_by_as_str_roundtrips() {
    for by in [
        RGroupBy::None,
        RGroupBy::Agent,
        RGroupBy::Machine,
        RGroupBy::Status,
        RGroupBy::Health,
    ] {
        assert_eq!(RGroupBy::from_str(by.as_str()), by);
    }
}

#[test]
fn r_group_by_default_is_none() {
    assert_eq!(RGroupBy::default(), RGroupBy::None);
}

#[test]
fn r_group_by_unknown_token_decodes_to_none() {
    assert_eq!(RGroupBy::from_str("bogus"), RGroupBy::None);
    assert_eq!(RGroupBy::from_str(""), RGroupBy::None);
}

// ── routine_group_key ─────────────────────────────────────────────────────────

#[test]
fn routine_group_key_agent_returns_agent_field() {
    let r = routine("id1", "t", "claude", "0 * * * *", &["m1"], &[], true);
    assert_eq!(routine_group_key(&r, RGroupBy::Agent), "claude");
}

#[test]
fn routine_group_key_machine_returns_first_machine() {
    let r = routine(
        "id1",
        "t",
        "claude",
        "0 * * * *",
        &["alpha", "beta"],
        &[],
        true,
    );
    assert_eq!(routine_group_key(&r, RGroupBy::Machine), "alpha");
}

#[test]
fn routine_group_key_machine_returns_unassigned_when_no_machines() {
    let r = routine("id1", "t", "claude", "0 * * * *", &[], &[], true);
    assert_eq!(routine_group_key(&r, RGroupBy::Machine), "(unassigned)");
}

#[test]
fn routine_group_key_status_enabled() {
    let r = routine("id1", "t", "claude", "0 * * * *", &[], &[], true);
    assert_eq!(routine_group_key(&r, RGroupBy::Status), "Enabled");
}

#[test]
fn routine_group_key_status_disabled() {
    let r = routine("id1", "t", "claude", "0 * * * *", &[], &[], false);
    assert_eq!(routine_group_key(&r, RGroupBy::Status), "Disabled");
}

#[test]
fn routine_group_key_none_returns_empty_string() {
    let r = routine("id1", "t", "claude", "0 * * * *", &[], &[], true);
    assert_eq!(routine_group_key(&r, RGroupBy::None), "");
}

// ── group_routines ────────────────────────────────────────────────────────────

#[test]
fn group_routines_none_returns_single_group_with_all_routines() {
    let rs = vec![
        routine("a", "t", "claude", "0 * * * *", &[], &[], true),
        routine("b", "t", "codex", "0 * * * *", &[], &[], false),
    ];
    let groups = group_routines(&rs, RGroupBy::None);
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].0, "");
    assert_eq!(groups[0].1.len(), 2);
}

#[test]
fn group_routines_by_agent_creates_one_group_per_agent() {
    let rs = vec![
        routine("a", "t", "claude", "0 * * * *", &[], &[], true),
        routine("b", "t", "codex", "0 * * * *", &[], &[], true),
        routine("c", "t", "claude", "0 * * * *", &[], &[], true),
    ];
    let groups = group_routines(&rs, RGroupBy::Agent);
    // BTreeMap → alphabetical: claude, codex
    assert_eq!(groups.len(), 2);
    assert_eq!(groups[0].0, "claude");
    assert_eq!(groups[0].1.len(), 2);
    assert_eq!(groups[1].0, "codex");
    assert_eq!(groups[1].1.len(), 1);
}

#[test]
fn group_routines_by_agent_preserves_input_order_within_group() {
    let rs = vec![
        routine("first", "t", "claude", "0 * * * *", &[], &[], true),
        routine("second", "t", "claude", "0 * * * *", &[], &[], true),
    ];
    let groups = group_routines(&rs, RGroupBy::Agent);
    assert_eq!(groups[0].1[0].id, "first");
    assert_eq!(groups[0].1[1].id, "second");
}

#[test]
fn group_routines_by_machine_separates_unassigned() {
    let rs = vec![
        routine("a", "t", "claude", "0 * * * *", &["worker-1"], &[], true),
        routine("b", "t", "claude", "0 * * * *", &[], &[], true),
        routine("c", "t", "claude", "0 * * * *", &["worker-1"], &[], true),
    ];
    let groups = group_routines(&rs, RGroupBy::Machine);
    // alphabetical: "(unassigned)" sorts before "worker-1"
    assert_eq!(groups.len(), 2);
    assert_eq!(groups[0].0, "(unassigned)");
    assert_eq!(groups[0].1.len(), 1);
    assert_eq!(groups[1].0, "worker-1");
    assert_eq!(groups[1].1.len(), 2);
}

#[test]
fn group_routines_by_status_splits_enabled_and_disabled() {
    let rs = vec![
        routine("a", "t", "claude", "0 * * * *", &[], &[], true),
        routine("b", "t", "claude", "0 * * * *", &[], &[], false),
        routine("c", "t", "claude", "0 * * * *", &[], &[], true),
    ];
    let groups = group_routines(&rs, RGroupBy::Status);
    // alphabetical: "Disabled" before "Enabled"
    assert_eq!(groups.len(), 2);
    assert_eq!(groups[0].0, "Disabled");
    assert_eq!(groups[0].1.len(), 1);
    assert_eq!(groups[1].0, "Enabled");
    assert_eq!(groups[1].1.len(), 2);
}

// ── sort by RCol::LastFire ────────────────────────────────────────────────────

fn routine_with_last_fire(id: &str, manual: Option<u64>, scheduled: Option<u64>) -> Routine {
    let mut r = routine(id, id, "claude", "0 * * * *", &[], &[], true);
    r.last_manual_trigger_at = manual;
    r.last_scheduled_trigger_at = scheduled;
    r
}

#[test]
fn sort_by_last_fire_ascending_puts_oldest_first() {
    let rs = vec![
        routine_with_last_fire("new", Some(300), None),
        routine_with_last_fire("old", Some(100), None),
        routine_with_last_fire("never", None, None),
    ];
    let sorted = sort_routines(rs, Some(RCol::LastFire), RDir::Asc, now());
    assert_eq!(sorted[0].id, "never");
    assert_eq!(sorted[1].id, "old");
    assert_eq!(sorted[2].id, "new");
}

#[test]
fn sort_by_last_fire_descending_puts_newest_first() {
    let rs = vec![
        routine_with_last_fire("old", Some(100), None),
        routine_with_last_fire("new", Some(300), None),
    ];
    let sorted = sort_routines(rs, Some(RCol::LastFire), RDir::Desc, now());
    assert_eq!(sorted[0].id, "new");
    assert_eq!(sorted[1].id, "old");
}

// ── CloseModal (Esc dismissal) ────────────────────────────────────────────────
// `install_search_hotkey` (routines/hooks.rs) dispatches this same action when
// Escape is pressed and a modal is open. These tests pin the reducer behavior
// it relies on: every modal variant resets to `None`, and no destructive/data
// action (delete, bulk-delete) ever fires as a side effect.

#[test]
fn close_modal_from_edit_resets_to_none() {
    let s = state_with_routines(&["a"]);
    let s = s.reduce(RAction::OpenEdit("a".into()));
    assert_eq!(s.modal, RModal::Edit("a".into()));
    let s = s.reduce(RAction::CloseModal);
    assert_eq!(s.modal, RModal::None);
}

#[test]
fn close_modal_from_confirm_delete_resets_to_none_without_deleting() {
    let s = state_with_routines(&["a", "b"]);
    let s = s.reduce(RAction::OpenConfirmDelete {
        id: "a".into(),
        title: "a".into(),
    });
    let s = s.reduce(RAction::CloseModal);
    assert_eq!(s.modal, RModal::None);
    // Esc must not act like a confirmed delete: both routines are still present.
    assert_eq!(s.routines.len(), 2);
}

#[test]
fn close_modal_from_confirm_bulk_delete_resets_to_none_without_deleting() {
    let s = state_with_routines(&["a", "b"]);
    let s = s.reduce(RAction::SelectAll(vec!["a".into(), "b".into()]));
    let s = s.reduce(RAction::OpenConfirmBulkDelete);
    let s = s.reduce(RAction::CloseModal);
    assert_eq!(s.modal, RModal::None);
    // Esc must not act like a confirmed bulk delete: selection and routines survive.
    assert_eq!(s.routines.len(), 2);
    assert_eq!(s.selected.len(), 2);
}

#[test]
fn close_modal_is_a_noop_when_no_modal_is_open() {
    let s = state_with_routines(&["a"]);
    assert_eq!(s.modal, RModal::None);
    let s = s.reduce(RAction::CloseModal);
    assert_eq!(s.modal, RModal::None);
}
