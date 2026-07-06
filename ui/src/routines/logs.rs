//! Routine logs page.

use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;

use crate::log_viewer::LogViewer;

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

    let on_back = {
        let cb = props.on_back.clone();
        Callback::from(move |_: MouseEvent| cb.emit(()))
    };
    let on_refresh = {
        let load = load.clone();
        Callback::from(move |_: MouseEvent| load())
    };

    let freshness = if *updated_at > 0.0 {
        let secs = ((js_sys::Date::now() - *updated_at).max(0.0) / 1000.0) as u64;
        crate::refresh::fmt_freshness(secs)
    } else {
        String::new()
    };

    html! {
        <main class="logs-page">
            <div class="page-hd">
                <button class="btn btn-ghost btn-sm" onclick={on_back}>{"← BACK"}</button>
                <div class="page-title">{format!("LOGS / {}", props.title)}</div>
                <span class="page-freshness">{freshness}</span>
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
