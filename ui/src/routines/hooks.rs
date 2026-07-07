//! Small custom Yew hooks used by the routines list page. Each installs one
//! self-contained mount-time effect (a timer loop or a one-shot fetch), keeping
//! `page.rs`'s component body to the wiring that actually varies per-render.

use std::cell::Cell;
use std::rc::Rc;

use chrono::{DateTime, Local};
use gloo_timers::future::TimeoutFuture;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::spawn_local;
use web_sys::{HtmlElement, HtmlInputElement, KeyboardEvent};
use yew::prelude::*;

use crate::machines::api_current_machine;
use crate::refresh::RefreshInterval;
use crate::ToastKind;

use super::model::{api_list, api_lock_status};
use super::state::{RAction, RModal, RState};

/// Tick cadence for the live "now" handle (keeps DueSoon count fresh between fetches).
const NEXT_RUN_TICK_MS: u32 = 30_000;

/// Installs the keydown listener behind two routines-page shortcuts: `/` focuses
/// `search_ref` (while not already typing in another field), matching the
/// GitHub/Slack convention and complementing the ⌘K palette; Escape dismisses
/// whichever routine modal/dialog is currently open. Torn down on unmount.
pub(crate) fn install_search_hotkey(search_ref: NodeRef, state: UseReducerHandle<RState>) {
    use_effect_with((), move |_| {
        let on_key =
            Closure::<dyn Fn(KeyboardEvent)>::wrap(Box::new(move |event: KeyboardEvent| {
                if event.key() == "Escape" {
                    if state.modal != RModal::None {
                        state.dispatch(RAction::CloseModal);
                    }
                    return;
                }
                if event.key() != "/" || event.meta_key() || event.ctrl_key() || event.alt_key() {
                    return;
                }
                let typing = event
                    .target()
                    .and_then(|t| t.dyn_into::<HtmlElement>().ok())
                    .is_some_and(|el| {
                        let tag = el.tag_name();
                        tag == "INPUT" || tag == "TEXTAREA" || tag == "SELECT"
                    });
                if typing {
                    return;
                }
                if let Some(input) = search_ref.cast::<HtmlInputElement>() {
                    event.prevent_default();
                    let _ = input.focus();
                }
            }));
        let window = web_sys::window().expect("window exists");
        window
            .add_event_listener_with_callback("keydown", on_key.as_ref().unchecked_ref())
            .expect("keydown listener attaches");
        move || {
            if let Some(window) = web_sys::window() {
                let _ = window.remove_event_listener_with_callback(
                    "keydown",
                    on_key.as_ref().unchecked_ref(),
                );
            }
            drop(on_key);
        }
    });
}

/// Advances `now` on a fixed tick so DUE SOON counts stay current between fetches.
pub(crate) fn install_now_ticker(now: UseStateHandle<DateTime<Local>>) {
    use_effect_with((), move |_| {
        spawn_local(async move {
            loop {
                TimeoutFuture::new(NEXT_RUN_TICK_MS).await;
                now.set(Local::now());
            }
        });
    });
}

/// Loads the routine list once on mount.
pub(crate) fn install_routines_loader(
    state: UseReducerHandle<RState>,
    toast: Callback<(String, ToastKind)>,
    updated_at: UseStateHandle<f64>,
) {
    use_effect_with((), move |_| {
        spawn_local(async move {
            match api_list().await {
                Ok(r) => {
                    state.dispatch(RAction::Loaded(r));
                    updated_at.set(js_sys::Date::now());
                }
                Err(e) => toast.emit((format!("Failed to load routines: {e}"), ToastKind::Err)),
            }
        });
    });
}

/// Fetches and applies the current machine as the default machine filter.
pub(crate) fn install_current_machine_loader(state: UseReducerHandle<RState>) {
    use_effect_with((), move |_| {
        spawn_local(async move {
            if let Ok(name) = api_current_machine().await {
                state.dispatch(RAction::CurrentMachineLoaded(name));
            }
        });
    });
}

/// Fetches the global lock status once on mount.
pub(crate) fn install_lock_status_loader(state: UseReducerHandle<RState>) {
    use_effect_with((), move |_| {
        spawn_local(async move {
            if let Ok(status) = api_lock_status().await {
                state.dispatch(RAction::LockStatusLoaded(status));
            }
        });
    });
}

/// Auto-refresh loop, re-armed whenever `interval` changes. `Off` installs no loop
/// (load-once behaviour); any cadence re-fetches the list on that period. The
/// cleanup flag stops the running loop when the interval changes or the page
/// unmounts.
pub(crate) fn install_auto_refresh(
    interval: RefreshInterval,
    state: UseReducerHandle<RState>,
    toast: Callback<(String, ToastKind)>,
    updated_at: UseStateHandle<f64>,
) {
    use_effect_with(interval, move |interval| {
        let cancelled = Rc::new(Cell::new(false));
        if let Some(period_ms) = interval.as_millis() {
            let cancelled = cancelled.clone();
            spawn_local(async move {
                loop {
                    TimeoutFuture::new(period_ms).await;
                    if cancelled.get() {
                        break;
                    }
                    match api_list().await {
                        Ok(r) => {
                            if cancelled.get() {
                                break;
                            }
                            state.dispatch(RAction::Loaded(r));
                            updated_at.set(js_sys::Date::now());
                        }
                        Err(e) => {
                            toast.emit((format!("Auto-refresh failed: {e}"), ToastKind::Err));
                        }
                    }
                }
            });
        }
        move || cancelled.set(true)
    });
}
