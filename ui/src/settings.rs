//! Settings page: the persistent system prompt appended to every routine's agent instructions,
//! and the global routine concurrency cap.
//!
//! Machine identity (rename) and the global schedule lock already have dedicated controls in the
//! header and on the Overview/Routines pages respectively; this page covers settings with no
//! other UI surface — `~/.config/moadim/user_prompt.md` and the `MOADIM_MAX_CONCURRENT_RUNS`
//! cap (issue #1155) were previously editable only via env var or by hand on disk.

use gloo_net::http::Request;
use serde::Deserialize;
use wasm_bindgen_futures::spawn_local;
use web_sys::{HtmlInputElement, HtmlTextAreaElement};
use yew::prelude::*;
use yew::TargetCast;

use crate::ToastKind;

async fn api_get_user_prompt() -> Result<String, String> {
    let resp = Request::get("/api/v1/config/user-prompt")
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.ok() {
        return Err(format!("HTTP {}", resp.status()));
    }
    resp.text().await.map_err(|e| e.to_string())
}

async fn api_put_user_prompt(content: &str) -> Result<(), String> {
    let resp = Request::put("/api/v1/config/user-prompt")
        .header("Content-Type", "application/json")
        .body(serde_json::json!({ "content": content }).to_string())
        .map_err(|e| e.to_string())?
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if resp.ok() {
        Ok(())
    } else {
        Err(format!("HTTP {}", resp.status()))
    }
}

/// Response shape shared by `GET`/`PUT /api/v1/config/max-concurrent-runs`. Only
/// `override_value` is read here — `value` (the fully-resolved, env-var-inclusive cap) isn't
/// meaningful to show as this input's value, see the field doc in `http_settings_routes.rs`.
#[derive(Deserialize)]
struct MaxConcurrentRunsResponse {
    override_value: Option<u64>,
}

async fn api_get_max_concurrent_runs_override() -> Result<Option<u64>, String> {
    let resp = Request::get("/api/v1/config/max-concurrent-runs")
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.ok() {
        return Err(format!("HTTP {}", resp.status()));
    }
    let body: MaxConcurrentRunsResponse = resp.json().await.map_err(|e| e.to_string())?;
    Ok(body.override_value)
}

async fn api_put_max_concurrent_runs_override(value: Option<u64>) -> Result<(), String> {
    let resp = Request::put("/api/v1/config/max-concurrent-runs")
        .header("Content-Type", "application/json")
        .body(serde_json::json!({ "value": value }).to_string())
        .map_err(|e| e.to_string())?
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if resp.ok() {
        Ok(())
    } else {
        Err(format!("HTTP {}", resp.status()))
    }
}

#[derive(Properties, PartialEq)]
pub struct SettingsPageProps {
    pub on_toast: Callback<(String, ToastKind)>,
}

#[function_component(SettingsPage)]
pub fn settings_page(props: &SettingsPageProps) -> Html {
    let content = use_state(String::new);
    let loaded_content = use_state(String::new);
    let loading = use_state(|| true);
    let saving = use_state(|| false);

    let cap_input = use_state(String::new);
    let loaded_cap = use_state(|| None::<u64>);
    let cap_loading = use_state(|| true);
    let cap_saving = use_state(|| false);

    {
        let content = content.clone();
        let loaded_content = loaded_content.clone();
        let loading = loading.clone();
        use_effect_with((), move |()| {
            spawn_local_load(content, loaded_content, loading);
        });
    }

    {
        let cap_input = cap_input.clone();
        let loaded_cap = loaded_cap.clone();
        let cap_loading = cap_loading.clone();
        use_effect_with((), move |()| {
            spawn_local_load_cap(cap_input, loaded_cap, cap_loading);
        });
    }

    let on_cap_input = {
        let cap_input = cap_input.clone();
        Callback::from(move |e: InputEvent| {
            if let Some(el) = e.target_dyn_into::<HtmlInputElement>() {
                cap_input.set(el.value());
            }
        })
    };

    let cap_dirty = parse_cap(&cap_input) != *loaded_cap;

    let on_save_cap = {
        let cap_input = cap_input.clone();
        let cap_saving = cap_saving.clone();
        let on_toast = props.on_toast.clone();
        Callback::from(move |_: MouseEvent| {
            let value = parse_cap(&cap_input);
            let loaded_cap = loaded_cap.clone();
            let cap_saving = cap_saving.clone();
            let on_toast = on_toast.clone();
            cap_saving.set(true);
            spawn_local(async move {
                match api_put_max_concurrent_runs_override(value).await {
                    Ok(()) => {
                        loaded_cap.set(value);
                        on_toast.emit(("Concurrency cap saved".into(), ToastKind::Ok));
                    }
                    Err(e) => on_toast.emit((format!("Save failed: {e}"), ToastKind::Err)),
                }
                cap_saving.set(false);
            });
        })
    };

    let on_input = {
        let content = content.clone();
        Callback::from(move |e: InputEvent| {
            if let Some(el) = e.target_dyn_into::<HtmlTextAreaElement>() {
                content.set(el.value());
            }
        })
    };

    let dirty = *content != *loaded_content;

    let on_save = {
        let content = content.clone();
        let saving = saving.clone();
        let on_toast = props.on_toast.clone();
        Callback::from(move |_: MouseEvent| {
            let content = content.clone();
            let loaded_content = loaded_content.clone();
            let saving = saving.clone();
            let on_toast = on_toast.clone();
            saving.set(true);
            spawn_local(async move {
                let text = (*content).clone();
                match api_put_user_prompt(&text).await {
                    Ok(()) => {
                        loaded_content.set(text);
                        on_toast.emit(("Prompt saved".into(), ToastKind::Ok));
                    }
                    Err(e) => on_toast.emit((format!("Save failed: {e}"), ToastKind::Err)),
                }
                saving.set(false);
            });
        })
    };

    html! {
        <main class="settings-page">
            <div class="section-hd">
                <div class="section-label">{"SETTINGS"}</div>
            </div>
            <div class="settings-card">
                <div class="settings-card-hd">{"PERSISTENT PROMPT"}</div>
                <p class="settings-card-sub">
                    {"Appended to every routine's agent instructions file (CLAUDE.md/AGENTS.md), \
                      alongside the moadim-managed preamble, on every run."}
                </p>
                if *loading {
                    <div class="empty"><div class="spinner"></div></div>
                } else {
                    <textarea
                        class="form-input settings-textarea"
                        rows="12"
                        placeholder="e.g. always run `cargo fmt` before finishing a task"
                        value={(*content).clone()}
                        oninput={on_input}
                    />
                    <div class="settings-card-acts">
                        <button
                            class="btn btn-primary btn-sm"
                            disabled={!dirty || *saving}
                            onclick={on_save}
                        >
                            { if *saving { "SAVING…" } else { "SAVE" } }
                        </button>
                        if dirty {
                            <span class="settings-dirty-hint">{"unsaved changes"}</span>
                        }
                    </div>
                }
            </div>
            <div class="settings-card">
                <div class="settings-card-hd">{"GLOBAL CONCURRENCY CAP"}</div>
                <p class="settings-card-sub">
                    {"Max routine agent sessions allowed to run at once before a new fire is \
                      skipped. Empty means unbounded. The MOADIM_MAX_CONCURRENT_RUNS env var, \
                      when set, overrides this."}
                </p>
                if *cap_loading {
                    <div class="empty"><div class="spinner"></div></div>
                } else {
                    <input
                        type="number"
                        min="0"
                        class="form-input"
                        placeholder="unbounded"
                        value={(*cap_input).clone()}
                        oninput={on_cap_input}
                    />
                    <div class="settings-card-acts">
                        <button
                            class="btn btn-primary btn-sm"
                            disabled={!cap_dirty || *cap_saving}
                            onclick={on_save_cap}
                        >
                            { if *cap_saving { "SAVING…" } else { "SAVE" } }
                        </button>
                        if cap_dirty {
                            <span class="settings-dirty-hint">{"unsaved changes"}</span>
                        }
                    </div>
                }
            </div>
        </main>
    }
}

/// Parse the concurrency-cap text input into the value that would be sent to the API: an empty
/// (or whitespace-only) field means "no override" (`None`); a non-empty field parses as `u64`.
/// `<input type="number">` already keeps the browser from producing non-numeric intermediate
/// values, so an unparsable non-empty string isn't reachable through normal UI interaction.
fn parse_cap(input: &str) -> Option<u64> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        None
    } else {
        trimmed.parse::<u64>().ok()
    }
}

/// Fetch the current user prompt and populate `content`/`loaded_content`, clearing `loading`.
fn spawn_local_load(
    content: UseStateHandle<String>,
    loaded_content: UseStateHandle<String>,
    loading: UseStateHandle<bool>,
) {
    spawn_local(async move {
        let text = api_get_user_prompt().await.unwrap_or_default();
        content.set(text.clone());
        loaded_content.set(text);
        loading.set(false);
    });
}

/// Fetch the persisted concurrency-cap override and populate `cap_input`/`loaded_cap`, clearing
/// `cap_loading`. A fetch error (or no override set) leaves the input blank ("unbounded").
fn spawn_local_load_cap(
    cap_input: UseStateHandle<String>,
    loaded_cap: UseStateHandle<Option<u64>>,
    cap_loading: UseStateHandle<bool>,
) {
    spawn_local(async move {
        let value = api_get_max_concurrent_runs_override().await.unwrap_or(None);
        cap_input.set(value.map_or_else(String::new, |v| v.to_string()));
        loaded_cap.set(value);
        cap_loading.set(false);
    });
}

#[cfg(test)]
#[path = "settings_tests.rs"]
mod settings_tests;
