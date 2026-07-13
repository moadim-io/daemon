//! Persisted "saved views" for the Routines page: the current filter, sort, and
//! group-by state can be captured into a portable `ViewSnapshot`, saved under a
//! name, and re-applied later. The most recent state is auto-persisted and
//! restored on load so a reload doesn't silently drop an operator's in-progress
//! triage view — matching the saved/pinned-view convention of dashboards an
//! operator revisits repeatedly (Linear, GitHub Issues, Grafana).
//!
//! The snapshot codec (`capture`/`decode`) is pure and host-tested; only the
//! `localStorage` round-trip and the `SavedViewsBar` component touch the
//! DOM/wasm layer (mirrors `refresh.rs`'s split).

use serde::{Deserialize, Serialize};
use web_sys::{HtmlInputElement, HtmlSelectElement};
use yew::prelude::*;

use super::filter::{
    AgentFacet, RepositoryFacet, RoutineFilter, RoutineMachineFacet, RoutineStatusFacet, TagFacet,
};
use super::state::{RCol, RDir, RGroupBy};

/// `localStorage` key for the list of named saved views.
const SAVED_VIEWS_KEY: &str = "moadim.routines.saved_views";
/// `localStorage` key for the most recently applied filter/sort/group-by state,
/// auto-restored on the next load.
const LAST_VIEW_KEY: &str = "moadim.routines.last_view";

/// Portable snapshot of the Routines page's filter, sort, and group-by state.
/// Every field is a plain string token (reusing each facet's existing
/// `as_str`/`as_value` codec), so this round-trips through JSON without
/// depending on enum discriminant stability.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ViewSnapshot {
    pub query: String,
    pub status: String,
    pub agent: String,
    pub machine: String,
    pub repository: String,
    pub tag: String,
    pub sort_col: Option<String>,
    pub sort_dir: String,
    pub group_by: String,
}

impl ViewSnapshot {
    /// Capture the given filter/sort/group-by state into a snapshot.
    #[must_use]
    pub fn capture(
        filter: &RoutineFilter,
        sort_col: Option<RCol>,
        sort_dir: RDir,
        group_by: RGroupBy,
    ) -> Self {
        Self {
            query: filter.query.clone(),
            status: filter.status.as_str().to_string(),
            agent: filter.agent.as_value(),
            machine: filter.machine.as_value(),
            repository: filter.repository.as_value(),
            tag: filter.tag.as_value(),
            sort_col: sort_col.map(|c| c.as_str().to_string()),
            sort_dir: sort_dir.as_str().to_string(),
            group_by: group_by.as_str().to_string(),
        }
    }
}

/// Decode a snapshot back into live filter/sort/group-by state. Unknown or
/// missing tokens fall back to each facet's default, so a snapshot from an
/// older build (or hand-edited `localStorage`) degrades gracefully instead of
/// failing to load.
#[must_use]
pub fn decode(snapshot: &ViewSnapshot) -> (RoutineFilter, Option<RCol>, RDir, RGroupBy) {
    let filter = RoutineFilter {
        query: snapshot.query.clone(),
        status: RoutineStatusFacet::from_str(&snapshot.status),
        agent: AgentFacet::from_value(&snapshot.agent),
        machine: RoutineMachineFacet::from_value(&snapshot.machine),
        repository: RepositoryFacet::from_value(&snapshot.repository),
        tag: TagFacet::from_value(&snapshot.tag),
    };
    let sort_col = snapshot.sort_col.as_deref().and_then(RCol::from_str);
    let sort_dir = RDir::from_str(&snapshot.sort_dir);
    let group_by = RGroupBy::from_str(&snapshot.group_by);
    (filter, sort_col, sort_dir, group_by)
}

/// A named, saved filter/sort/group-by preset.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedView {
    pub name: String,
    pub snapshot: ViewSnapshot,
}

/// Load the saved-view list from `localStorage`, defaulting to empty when
/// storage is unavailable or holds no/garbage value.
#[must_use]
pub fn load_saved_views() -> Vec<SavedView> {
    web_sys::window()
        .and_then(|win| win.local_storage().ok().flatten())
        .and_then(|store| store.get_item(SAVED_VIEWS_KEY).ok().flatten())
        .and_then(|json| serde_json::from_str(&json).ok())
        .unwrap_or_default()
}

/// Persist the saved-view list to `localStorage`. Best-effort: a storage error
/// (e.g. private-mode quota) is silently ignored — the in-memory list still
/// applies for the session.
pub fn save_saved_views(views: &[SavedView]) {
    if let Some(store) = web_sys::window().and_then(|win| win.local_storage().ok().flatten()) {
        if let Ok(json) = serde_json::to_string(views) {
            let _ = store.set_item(SAVED_VIEWS_KEY, &json);
        }
    }
}

/// Load the last-applied filter/sort/group-by snapshot, if any was persisted.
#[must_use]
pub fn load_last_view() -> Option<ViewSnapshot> {
    web_sys::window()
        .and_then(|win| win.local_storage().ok().flatten())
        .and_then(|store| store.get_item(LAST_VIEW_KEY).ok().flatten())
        .and_then(|json| serde_json::from_str(&json).ok())
}

/// Persist the current filter/sort/group-by snapshot as the one to restore on
/// next load. Best-effort, mirrors `save_saved_views`.
pub fn save_last_view(snapshot: &ViewSnapshot) {
    if let Some(store) = web_sys::window().and_then(|win| win.local_storage().ok().flatten()) {
        if let Ok(json) = serde_json::to_string(snapshot) {
            let _ = store.set_item(LAST_VIEW_KEY, &json);
        }
    }
}

// ─── Saved views bar ───────────────────────────────────────────────────────────

#[derive(Properties, PartialEq)]
pub struct SavedViewsBarProps {
    pub views: Vec<SavedView>,
    pub on_apply: Callback<ViewSnapshot>,
    pub on_save: Callback<String>,
    pub on_delete: Callback<String>,
}

/// Dropdown to apply a saved view, plus inline controls to save the current
/// filter/sort/group-by state as a new named view or delete the picked one.
#[function_component(SavedViewsBar)]
pub fn saved_views_bar(props: &SavedViewsBarProps) -> Html {
    let editing = use_state(|| false);
    let draft = use_state(String::new);
    // ponytail: tracks only which view was last picked/saved for the DELETE button and
    // the select's displayed value — it does not clear when the operator edits a filter
    // by hand afterwards, so the label can point at a view the current state has since
    // diverged from. Harmless (re-picking or re-saving fixes it) and not worth a diff
    // that watches every filter field just to blank one label.
    let picked = use_state(String::new);

    let on_pick = {
        let on_apply = props.on_apply.clone();
        let views = props.views.clone();
        let picked = picked.clone();
        Callback::from(move |e: Event| {
            let select: HtmlSelectElement = e.target_unchecked_into();
            let name = select.value();
            if let Some(view) = views.iter().find(|v| v.name == name) {
                on_apply.emit(view.snapshot.clone());
            }
            picked.set(name);
        })
    };

    let on_start_save = {
        let editing = editing.clone();
        let draft = draft.clone();
        Callback::from(move |_: MouseEvent| {
            editing.set(true);
            draft.set(String::new());
        })
    };

    let on_draft_input = {
        let draft = draft.clone();
        Callback::from(move |e: InputEvent| {
            let input: HtmlInputElement = e.target_unchecked_into();
            draft.set(input.value());
        })
    };

    let on_confirm_save = {
        let cb = props.on_save.clone();
        let draft = draft.clone();
        let editing = editing.clone();
        let picked = picked.clone();
        Callback::from(move |_: MouseEvent| {
            let name = draft.trim().to_string();
            if name.is_empty() {
                return;
            }
            cb.emit(name.clone());
            picked.set(name);
            editing.set(false);
        })
    };

    let on_cancel_save = {
        let editing = editing.clone();
        Callback::from(move |_: MouseEvent| editing.set(false))
    };

    let on_delete_click = {
        let cb = props.on_delete.clone();
        let picked = picked.clone();
        Callback::from(move |_: MouseEvent| {
            if picked.is_empty() {
                return;
            }
            cb.emit((*picked).clone());
            picked.set(String::new());
        })
    };

    let picked_val = (*picked).clone();
    let draft_empty = draft.trim().is_empty();

    html! {
        <div class="filter-bar saved-views-bar">
            <div class="filter-field">
                <span class="filter-label">{"VIEWS"}</span>
                <select class="filter-select" aria-label="Saved views" onchange={on_pick}>
                    <option value="" selected={picked_val.is_empty()}>{"— select —"}</option>
                    { for props.views.iter().map(|v| html! {
                        <option value={v.name.clone()} selected={picked_val == v.name}>
                            {v.name.clone()}
                        </option>
                    }) }
                </select>
                {
                    if *editing {
                        html! {
                            <>
                                <input
                                    type="text"
                                    class="filter-input"
                                    placeholder="View name…"
                                    aria-label="New view name"
                                    value={(*draft).clone()}
                                    oninput={on_draft_input}
                                />
                                <button class="btn btn-primary btn-sm" onclick={on_confirm_save}
                                    disabled={draft_empty}>{"SAVE"}</button>
                                <button class="btn btn-ghost btn-sm" onclick={on_cancel_save}>
                                    {"CANCEL"}
                                </button>
                            </>
                        }
                    } else {
                        html! {
                            <>
                                <button class="btn btn-ghost btn-sm" onclick={on_start_save}
                                    title="Save current filters, sort, and grouping as a named view">
                                    {"☆ SAVE VIEW"}
                                </button>
                                {
                                    if picked_val.is_empty() {
                                        html! {}
                                    } else {
                                        html! {
                                            <button class="btn btn-ghost btn-sm" onclick={on_delete_click}
                                                title="Delete this saved view">{"DELETE"}</button>
                                        }
                                    }
                                }
                            </>
                        }
                    }
                }
            </div>
        </div>
    }
}

#[cfg(test)]
#[path = "saved_views_tests.rs"]
mod saved_views_tests;
