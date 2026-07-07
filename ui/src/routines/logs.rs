//! Routine logs page.

use std::cell::Cell;
use std::rc::Rc;

use gloo_timers::future::TimeoutFuture;
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;

use crate::log_viewer::LogViewer;
use crate::refresh::{RefreshControl, RefreshInterval};

use super::model::api_logs;

// ─── Logs page ────────────────────────────────────────────────────────────────

#[derive(Properties, PartialEq)]
pub struct LogsProps {
    pub id: String,
    pub title: String,
    pub on_back: Callback<()>,
}

#[function_component(RoutineLogs)]
pub fn routine_logs(props: &LogsProps) -> Html {
    let content: UseStateHandle<Option<String>> = use_state(|| None);
    let loading = use_state(|| true);
    let err: UseStateHandle<Option<String>> = use_state(|| None);
    let updated_at: UseStateHandle<f64> = use_state(|| 0.0);

    let load = {
        let id = props.id.clone();
        let content = content.clone();
        let loading = loading.clone();
        let err = err.clone();
        let updated_at = updated_at.clone();
        move || {
            let id = id.clone();
            let content = content.clone();
            let loading = loading.clone();
            let err = err.clone();
            let updated_at = updated_at.clone();
            loading.set(true);
            spawn_local(async move {
                match api_logs(&id).await {
                    Ok(text) => {
                        content.set(Some(text));
                        err.set(None);
                        updated_at.set(js_sys::Date::now());
                    }
                    Err(e) => err.set(Some(e)),
                }
                loading.set(false);
            });
        }
    };

    {
        let load = load.clone();
        use_effect_with(props.id.clone(), move |_| {
            load();
        });
    }

    // Operator-chosen auto-refresh cadence (shared with the routines list page via
    // `localStorage`), re-armed whenever it changes. Without this, a workbench reaped by the
    // periodic backend cleanup sweep while this page is open leaves stale, already-deleted run
    // output on screen until the operator remembers to hit the manual "↻" button (#357).
    let interval = use_state(crate::refresh::load_interval);
    {
        let load = load.clone();
        use_effect_with(*interval, move |interval| {
            let cancelled = Rc::new(Cell::new(false));
            if let Some(period_ms) = interval.as_millis() {
                let cancelled = cancelled.clone();
                let load = load.clone();
                spawn_local(async move {
                    loop {
                        TimeoutFuture::new(period_ms).await;
                        if cancelled.get() {
                            break;
                        }
                        load();
                    }
                });
            }
            move || cancelled.set(true)
        });
    }
    let on_set_interval = {
        let interval = interval.clone();
        Callback::from(move |next: RefreshInterval| {
            interval.set(next);
            crate::refresh::save_interval(next);
        })
    };

    let on_back = {
        let cb = props.on_back.clone();
        Callback::from(move |_: MouseEvent| cb.emit(()))
    };
    let on_refresh = {
        let load = load.clone();
        Callback::from(move |_: MouseEvent| load())
    };

    html! {
        <main class="logs-page">
            <div class="page-hd">
                <button class="btn btn-ghost btn-sm" onclick={on_back}>{"← BACK"}</button>
                <div class="page-title">{format!("LOGS / {}", props.title)}</div>
                <RefreshControl
                    interval={*interval}
                    updated_at_ms={*updated_at}
                    on_change={on_set_interval}
                />
                <button class="btn-refresh" title="Refresh" aria-label="Refresh" onclick={on_refresh}>{"↻"}</button>
            </div>
            <LogViewer
                content={(*content).clone()}
                loading={*loading}
                err={(*err).clone()}
            />
        </main>
    }
}
