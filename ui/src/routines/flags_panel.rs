//! Routine flags page: lists open flags raised by the agent and lets the
//! operator resolve them.

use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;

use super::model::{api_flags, api_resolve_flag, Flag, FlagScope};

// ─── Flags page ───────────────────────────────────────────────────────────────

#[derive(Properties, PartialEq)]
pub struct FlagsProps {
    pub id: String,
    pub title: String,
    pub on_back: Callback<()>,
}

#[function_component(RoutineFlags)]
pub fn routine_flags(props: &FlagsProps) -> Html {
    let flags: UseStateHandle<Vec<Flag>> = use_state(Vec::new);
    let loading = use_state(|| true);
    let err: UseStateHandle<Option<String>> = use_state(|| None);
    let updated_at: UseStateHandle<f64> = use_state(|| 0.0);

    let load = {
        let id = props.id.clone();
        let flags = flags.clone();
        let loading = loading.clone();
        let err = err.clone();
        let updated_at = updated_at.clone();
        move || {
            let id = id.clone();
            let flags = flags.clone();
            let loading = loading.clone();
            let err = err.clone();
            let updated_at = updated_at.clone();
            loading.set(true);
            spawn_local(async move {
                match api_flags(&id).await {
                    Ok(list) => {
                        flags.set(list);
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

    let body = if *loading {
        html! { <div class="empty"><div class="spinner"></div></div> }
    } else if let Some(msg) = (*err).clone() {
        html! { <div class="logs-error">{msg}</div> }
    } else if flags.is_empty() {
        html! {
            <div class="empty">
                <div class="empty-icon">{"⚑"}</div>
                <div class="empty-msg">{"NO OPEN FLAGS"}</div>
            </div>
        }
    } else {
        let id = props.id.clone();
        let count = flags.len();
        html! {
            <div class="flags-list">
                <div class="flags-count">{format!("{count} open flag{}", if count == 1 { "" } else { "s" })}</div>
                { for flags.iter().map(|flag| {
                    let on_resolve = {
                        let id = id.clone();
                        let filename = flag.filename.clone();
                        let load = load.clone();
                        Callback::from(move |_: MouseEvent| {
                            let id = id.clone();
                            let filename = filename.clone();
                            let load = load.clone();
                            spawn_local(async move {
                                if api_resolve_flag(&id, &filename).await.is_ok() {
                                    load();
                                }
                            });
                        })
                    };
                    let scope_label = match flag.scope {
                        FlagScope::General => "general",
                        FlagScope::Local => "local",
                    };
                    let age = crate::reltime(flag.created_at);
                    html! {
                        <div class="flag-item" key={flag.filename.clone()}>
                            <div class="flag-item-hd">
                                <span class="flag-type">{&flag.flag_type}</span>
                                <span class="flag-scope">{scope_label}</span>
                                <span class="flag-age" title={flag.filename.clone()}>{age}</span>
                                <button class="btn btn-ghost btn-sm" onclick={on_resolve}>{"RESOLVE"}</button>
                            </div>
                            <div class="flag-desc">{&flag.description}</div>
                        </div>
                    }
                }) }
            </div>
        }
    };

    let flags_freshness = if *updated_at > 0.0 {
        let secs = ((js_sys::Date::now() - *updated_at).max(0.0) / 1000.0) as u64;
        crate::refresh::fmt_freshness(secs)
    } else {
        String::new()
    };

    html! {
        <main class="logs-page">
            <div class="page-hd">
                <button class="btn btn-ghost btn-sm" onclick={on_back}>{"← BACK"}</button>
                <div class="page-title">{format!("FLAGS / {}", props.title)}</div>
                <span class="page-freshness">{flags_freshness}</span>
                <button class="btn-refresh" title="Refresh" aria-label="Refresh" onclick={on_refresh}>{"↻"}</button>
            </div>
            {body}
        </main>
    }
}
