//! Settings page: the persistent system prompt appended to every routine's agent instructions.
//!
//! Machine identity (rename) and the global schedule lock already have dedicated controls in the
//! header and on the Overview/Routines pages respectively; this page covers the one setting with
//! no existing UI surface — `~/.config/moadim/user_prompt.md` was previously editable only by
//! hand on disk.

use gloo_net::http::Request;
use wasm_bindgen_futures::spawn_local;
use web_sys::HtmlTextAreaElement;
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

    {
        let content = content.clone();
        let loaded_content = loaded_content.clone();
        let loading = loading.clone();
        use_effect_with((), move |_| {
            spawn_local_load(content, loaded_content, loading);
        });
    }

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
        let loaded_content = loaded_content.clone();
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
        </main>
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
