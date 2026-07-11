//! CRUD/API action callbacks for the routines list page: unlock-all, create, cleanup,
//! trigger, toggle enabled, save (create/update via the edit modal), and confirm-delete.
//! Each performs the `/routines` API call and updates reducer state / toasts on completion.

use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;

use crate::ToastKind;

use super::model::{
    api_cleanup, api_create, api_delete, api_trigger, api_unlock, api_update, humanize_bytes,
    CreateRoutineRequest, UpdateRoutineRequest,
};
use super::state::{RAction, RModal, RState};

/// CRUD/API callbacks wired up for the routines list page.
pub(crate) struct CrudHandlers {
    pub on_unlock_all: Callback<MouseEvent>,
    pub on_create: Callback<CreateRoutineRequest>,
    pub on_cleanup: Callback<MouseEvent>,
    pub on_trigger: Callback<String>,
    pub on_toggle: Callback<(String, bool)>,
    pub on_save: Callback<CreateRoutineRequest>,
    pub on_confirm_delete: Callback<String>,
}

/// Builds the routines list page's CRUD/API callbacks (unlock-all, create, cleanup,
/// trigger, toggle, save, confirm-delete) against the `/routines` API.
pub(crate) fn install_crud_handlers(
    state: UseReducerHandle<RState>,
    toast: Callback<(String, ToastKind)>,
    current_modal: RModal,
) -> CrudHandlers {
    let ok_toast = {
        let toast = toast.clone();
        move |msg: &str| toast.emit((msg.to_string(), ToastKind::Ok))
    };

    let on_unlock_all = {
        let state = state.clone();
        let toast = toast.clone();
        let ok = ok_toast.clone();
        Callback::from(move |_: MouseEvent| {
            let state = state.clone();
            let toast = toast.clone();
            let ok = ok.clone();
            spawn_local(async move {
                match api_unlock("all").await {
                    Ok(status) => {
                        state.dispatch(RAction::LockStatusLoaded(status));
                        ok("Routines unlocked");
                    }
                    Err(err_msg) => {
                        toast.emit((format!("Unlock failed: {err_msg}"), ToastKind::Err));
                    }
                }
            });
        })
    };

    let on_create = {
        let state = state.clone();
        let toast = toast.clone();
        let ok = ok_toast.clone();
        Callback::from(move |req: CreateRoutineRequest| {
            let state = state.clone();
            let toast = toast.clone();
            let ok = ok.clone();
            spawn_local(async move {
                match api_create(&req).await {
                    Ok(r) => {
                        state.dispatch(RAction::Upsert(Box::new(r)));
                        state.dispatch(RAction::GoToList);
                        ok("Routine created");
                    }
                    Err(e) => toast.emit((format!("Create failed: {e}"), ToastKind::Err)),
                }
            });
        })
    };

    let on_cleanup = {
        let toast = toast.clone();
        let ok = ok_toast.clone();
        Callback::from(move |_: MouseEvent| {
            let toast = toast.clone();
            let ok = ok.clone();
            spawn_local(async move {
                match api_cleanup().await {
                    Ok((n, freed_bytes)) => ok(&format!(
                        "Cleanup removed {n} workbench{} (freed {})",
                        if n == 1 { "" } else { "es" },
                        humanize_bytes(freed_bytes)
                    )),
                    Err(e) => toast.emit((format!("Cleanup failed: {e}"), ToastKind::Err)),
                }
            });
        })
    };

    let on_trigger = {
        let state = state.clone();
        let toast = toast.clone();
        let ok = ok_toast.clone();
        Callback::from(move |id: String| {
            let state = state.clone();
            let toast = toast.clone();
            let ok = ok.clone();
            spawn_local(async move {
                match api_trigger(&id).await {
                    Ok(r) => {
                        state.dispatch(RAction::Upsert(Box::new(r)));
                        ok("Routine triggered");
                    }
                    Err(e) => toast.emit((format!("Trigger failed: {e}"), ToastKind::Err)),
                }
            });
        })
    };

    let on_toggle = {
        let state = state.clone();
        let toast = toast.clone();
        let ok = ok_toast.clone();
        Callback::from(move |(id, enabled): (String, bool)| {
            let state = state.clone();
            let toast = toast.clone();
            let ok = ok.clone();
            spawn_local(async move {
                let req = UpdateRoutineRequest {
                    enabled: Some(enabled),
                    ..Default::default()
                };
                match api_update(&id, &req).await {
                    Ok(r) => {
                        state.dispatch(RAction::Upsert(Box::new(r)));
                        ok(if enabled {
                            "Routine enabled"
                        } else {
                            "Routine disabled"
                        });
                    }
                    Err(e) => toast.emit((format!("Toggle failed: {e}"), ToastKind::Err)),
                }
            });
        })
    };

    let on_save = {
        let state = state.clone();
        let toast = toast.clone();
        let ok = ok_toast.clone();
        Callback::from(move |req: CreateRoutineRequest| {
            let state = state.clone();
            let toast = toast.clone();
            let ok = ok.clone();
            let modal = current_modal.clone();
            spawn_local(async move {
                if let RModal::Edit(id) = &modal {
                    let upd = UpdateRoutineRequest {
                        schedule: Some(req.schedule),
                        title: Some(req.title),
                        agent: Some(req.agent),
                        // Always send the model so clearing the field (empty string) clears it server-side.
                        model: Some(req.model.unwrap_or_default()),
                        prompt: Some(req.prompt),
                        // Always send the goal so clearing the field (empty string) clears it server-side.
                        goal: Some(req.goal.unwrap_or_default()),
                        repositories: Some(req.repositories),
                        machines: Some(req.machines),
                        enabled: Some(req.enabled),
                        ttl_secs: req.ttl_secs,
                        tags: Some(req.tags),
                    };
                    match api_update(id, &upd).await {
                        Ok(r) => {
                            state.dispatch(RAction::Upsert(Box::new(r)));
                            state.dispatch(RAction::CloseModal);
                            ok("Routine updated");
                        }
                        Err(e) => toast.emit((format!("Update failed: {e}"), ToastKind::Err)),
                    }
                }
            });
        })
    };

    let on_confirm_delete = {
        let state = state.clone();
        let toast = toast.clone();
        let ok = ok_toast.clone();
        Callback::from(move |id: String| {
            let state = state.clone();
            let toast = toast.clone();
            let ok = ok.clone();
            spawn_local(async move {
                match api_delete(&id).await {
                    Ok(()) => {
                        state.dispatch(RAction::Remove(id));
                        state.dispatch(RAction::CloseModal);
                        ok("Routine deleted");
                    }
                    Err(e) => toast.emit((format!("Delete failed: {e}"), ToastKind::Err)),
                }
            });
        })
    };

    CrudHandlers {
        on_unlock_all,
        on_create,
        on_cleanup,
        on_trigger,
        on_toggle,
        on_save,
        on_confirm_delete,
    }
}
