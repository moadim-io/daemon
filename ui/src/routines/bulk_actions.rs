//! Bulk-selection callbacks for the routines list page: select/select-all/clear,
//! and bulk enable/disable/delete against the `/routines` API.

use chrono::{DateTime, Duration, Local};
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;

use crate::ToastKind;

use super::filter::{filter_routines, DUE_SOON_WINDOW_SECS};
use super::model::{api_delete, api_update, UpdateRoutineRequest};
use super::state::{RAction, RState};

/// Bulk-selection callbacks wired up for the routines list page.
pub(crate) struct BulkHandlers {
    pub on_select: Callback<String>,
    pub on_select_all: Callback<()>,
    pub on_clear_selection: Callback<()>,
    pub on_bulk_enable: Callback<()>,
    pub on_bulk_disable: Callback<()>,
    pub on_bulk_delete: Callback<()>,
    pub on_confirm_bulk_delete: Callback<()>,
}

/// Builds the bulk-selection action bar's callbacks (select/select-all/clear,
/// and bulk enable/disable/delete against the `/routines` API).
pub(crate) fn install_bulk_handlers(
    state: &UseReducerHandle<RState>,
    toast: &Callback<(String, ToastKind)>,
    now: &UseStateHandle<DateTime<Local>>,
) -> BulkHandlers {
    let on_select = {
        let state = state.clone();
        Callback::from(move |id: String| state.dispatch(RAction::SelectRoutine(id)))
    };

    // Header checkbox: toggle "all visible selected ↔ none" (filter-scoped).
    let on_select_all = {
        let state = state.clone();
        let now = now.clone();
        Callback::from(move |(): ()| {
            let window = Duration::seconds(DUE_SOON_WINDOW_SECS);
            let visible = filter_routines(&state.routines, &state.filter, *now, window);
            let all_visible_selected =
                !visible.is_empty() && visible.iter().all(|r| state.selected.contains(&r.id));
            if all_visible_selected {
                state.dispatch(RAction::ClearSelection);
            } else {
                state.dispatch(RAction::SelectAll(
                    visible.into_iter().map(|r| r.id).collect(),
                ));
            }
        })
    };

    let on_clear_selection = {
        let state = state.clone();
        Callback::from(move |(): ()| state.dispatch(RAction::ClearSelection))
    };

    // Bulk enable/disable: PATCH each selected routine, surface one summary toast.
    let bulk_set_enabled = {
        let state = state.clone();
        let toast = toast.clone();
        move |enabled: bool| {
            let state = state.clone();
            let toast = toast.clone();
            let ids: Vec<String> = state.selected.iter().cloned().collect();
            if ids.is_empty() {
                return;
            }
            spawn_local(async move {
                let mut ok = 0_usize;
                let mut failed = 0_usize;
                for id in ids {
                    let req = UpdateRoutineRequest {
                        enabled: Some(enabled),
                        ..Default::default()
                    };
                    match api_update(&id, &req).await {
                        Ok(r) => {
                            state.dispatch(RAction::Upsert(Box::new(r)));
                            ok += 1;
                        }
                        Err(_) => failed += 1,
                    }
                }
                let verb = if enabled { "enabled" } else { "disabled" };
                if failed == 0 {
                    toast.emit((format!("{ok} routine(s) {verb}"), ToastKind::Ok));
                } else {
                    toast.emit((format!("{ok} {verb}, {failed} failed"), ToastKind::Err));
                }
            });
        }
    };

    let on_bulk_enable = {
        let f = bulk_set_enabled.clone();
        Callback::from(move |(): ()| f(true))
    };
    let on_bulk_disable = { Callback::from(move |(): ()| bulk_set_enabled(false)) };

    let on_bulk_delete = {
        let state = state.clone();
        Callback::from(move |(): ()| state.dispatch(RAction::OpenConfirmBulkDelete))
    };

    let on_confirm_bulk_delete = {
        let state = state.clone();
        let toast = toast.clone();
        Callback::from(move |(): ()| {
            let state = state.clone();
            let toast = toast.clone();
            let ids: Vec<String> = state.selected.iter().cloned().collect();
            spawn_local(async move {
                let mut ok = 0_usize;
                let mut failed = 0_usize;
                let mut deleted: Vec<String> = Vec::new();
                for id in ids {
                    match api_delete(&id).await {
                        Ok(()) => {
                            deleted.push(id);
                            ok += 1;
                        }
                        Err(_) => failed += 1,
                    }
                }
                state.dispatch(RAction::RemoveMany(deleted));
                state.dispatch(RAction::CloseModal);
                if failed == 0 {
                    toast.emit((format!("{ok} routine(s) deleted"), ToastKind::Ok));
                } else {
                    toast.emit((format!("{ok} deleted, {failed} failed"), ToastKind::Err));
                }
            });
        })
    };

    BulkHandlers {
        on_select,
        on_select_all,
        on_clear_selection,
        on_bulk_enable,
        on_bulk_disable,
        on_bulk_delete,
        on_confirm_bulk_delete,
    }
}
