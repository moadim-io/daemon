//! Shell-level dialogs, chrome, and small API client used by `main.rs`'s `Shell`
//! component: the stop-server confirmation, the rename-machine dialog, the toast
//! stack, and the health/shutdown/machine-name API calls behind them.

use gloo_net::http::Request;
use yew::prelude::*;

use crate::{Health, ShellAction, ShellState, Toast, ToastKind};

// ─── API layer ────────────────────────────────────────────────────────────────

pub(crate) async fn api_health() -> Result<Health, String> {
    Request::get("/api/v1/health")
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json::<Health>()
        .await
        .map_err(|e| e.to_string())
}

pub(crate) async fn api_shutdown() -> Result<(), String> {
    let resp = Request::post("/api/v1/shutdown")
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if resp.ok() {
        Ok(())
    } else {
        Err(format!("HTTP {}", resp.status()))
    }
}

pub(crate) async fn api_get_machine() -> Result<String, String> {
    #[derive(serde::Deserialize)]
    struct Resp {
        name: String,
    }
    Request::get("/api/v1/machine")
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json::<Resp>()
        .await
        .map(|r| r.name)
        .map_err(|e| e.to_string())
}

pub(crate) async fn api_put_machine(name: &str) -> Result<String, String> {
    #[derive(serde::Serialize)]
    struct Body<'a> {
        name: &'a str,
    }
    #[derive(serde::Deserialize)]
    struct Resp {
        name: String,
    }
    let resp = Request::put("/api/v1/machine")
        .header("content-type", "application/json")
        .body(serde_json::to_string(&Body { name }).unwrap())
        .map_err(|e| e.to_string())?
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if resp.ok() {
        resp.json::<Resp>()
            .await
            .map(|r| r.name)
            .map_err(|e| e.to_string())
    } else {
        Err(format!("HTTP {}", resp.status()))
    }
}

pub(crate) async fn poll_health(state: UseReducerHandle<ShellState>) {
    match api_health().await {
        Ok(health) => {
            let ok = health.running;
            state.dispatch(ShellAction::HealthLoaded { health, ok });
        }
        Err(_) => state.dispatch(ShellAction::HealthLoaded {
            health: Health {
                status: "offline".into(),
                running: false,
                uptime_secs: None,
                version: None,
                git_sha: None,
                dependencies: None,
            },
            ok: false,
        }),
    }
}

// ─── Shutdown confirm dialog ──────────────────────────────────────────────────

#[derive(Properties, PartialEq)]
pub struct ShutdownProps {
    pub on_cancel: Callback<()>,
    pub on_confirm: Callback<()>,
}

#[function_component(ShutdownDialog)]
pub fn shutdown_dialog(props: &ShutdownProps) -> Html {
    let on_cancel = {
        let cb = props.on_cancel.clone();
        Callback::from(move |_: MouseEvent| cb.emit(()))
    };
    let on_confirm = {
        let cb = props.on_confirm.clone();
        Callback::from(move |_: MouseEvent| cb.emit(()))
    };

    html! {
        <div class="overlay open">
            <div
                class="confirm-dialog"
                role="dialog"
                aria-modal="true"
                aria-labelledby="shutdown-dialog-title"
                aria-describedby="shutdown-dialog-msg"
            >
                <div id="shutdown-dialog-title" class="confirm-title">{"⏻ STOP SERVER"}</div>
                <div id="shutdown-dialog-msg" class="confirm-msg">
                    { "Stop the moadim server? Scheduled jobs and routines will not run until it is started again." }
                </div>
                <div class="confirm-acts">
                    <button class="btn btn-ghost btn-sm" onclick={on_cancel}>{"CANCEL"}</button>
                    <button class="btn btn-danger btn-sm" onclick={on_confirm}>{"STOP SERVER"}</button>
                </div>
            </div>
        </div>
    }
}

// ─── Rename machine dialog ────────────────────────────────────────────────────

#[derive(Properties, PartialEq)]
pub struct RenameMachineProps {
    /// Current machine name (pre-fills the input).
    pub current: String,
    pub on_cancel: Callback<()>,
    /// Emits `(new_name, done_callback)`. The caller fires the API call; the `done_callback` is
    /// invoked with `Ok(())` on success so the dialog can reset its busy state.
    pub on_confirm: Callback<(String, Callback<Result<(), String>>)>,
}

#[function_component(RenameMachineDialog)]
pub fn rename_machine_dialog(props: &RenameMachineProps) -> Html {
    let draft = use_state(|| props.current.clone());
    let busy = use_state(|| false);

    let on_input = {
        let draft = draft.clone();
        Callback::from(move |e: InputEvent| {
            let input: web_sys::HtmlInputElement = e.target_unchecked_into();
            draft.set(input.value());
        })
    };

    let on_cancel = {
        let cb = props.on_cancel.clone();
        Callback::from(move |_: MouseEvent| cb.emit(()))
    };

    let on_save = {
        let draft = draft.clone();
        let busy = busy.clone();
        let cb = props.on_confirm.clone();
        Callback::from(move |_: MouseEvent| {
            let name = (*draft).trim().to_string();
            if name.is_empty() {
                return;
            }
            busy.set(true);
            let busy2 = busy.clone();
            let done = Callback::from(move |_: Result<(), String>| {
                busy2.set(false);
            });
            cb.emit((name, done));
        })
    };

    let is_busy = *busy;
    let is_empty = draft.trim().is_empty();

    html! {
        <div class="overlay open">
            <div
                class="confirm-dialog"
                role="dialog"
                aria-modal="true"
                aria-labelledby="rename-machine-title"
            >
                <div id="rename-machine-title" class="confirm-title">{"RENAME MACHINE"}</div>
                <div class="confirm-msg">
                    <label class="form-label" for="rename-machine-input">{"MACHINE NAME"}</label>
                    <input id="rename-machine-input"
                        class="form-input"
                        type="text"
                        value={(*draft).clone()}
                        oninput={on_input}
                        disabled={is_busy}
                        autocomplete="off"
                        spellcheck="false"
                    />
                </div>
                <div class="confirm-acts">
                    <button class="btn btn-ghost btn-sm" onclick={on_cancel} disabled={is_busy}>{"CANCEL"}</button>
                    <button class="btn btn-primary btn-sm" onclick={on_save}
                        disabled={is_busy || is_empty}>
                        { if is_busy { "SAVING…" } else { "RENAME" } }
                    </button>
                </div>
            </div>
        </div>
    }
}

// ─── Toast stack ──────────────────────────────────────────────────────────────

#[derive(Properties, PartialEq)]
pub struct ToastStackProps {
    pub toasts: Vec<Toast>,
}

#[function_component(ToastStack)]
pub fn toast_stack(props: &ToastStackProps) -> Html {
    html! {
        <div class="toast-wrap" role="status" aria-live="polite" aria-atomic="false">
            { for props.toasts.iter().map(|t| {
                let cls = match t.kind { ToastKind::Ok => "toast ok", ToastKind::Err => "toast err" };
                html! {
                    <div class={cls} key={t.id}>{t.msg.clone()}</div>
                }
            }) }
        </div>
    }
}

/// Formats an uptime duration for the header (`"42s"`, `"3m"`, `"1h 4m"`).
pub(crate) fn fmt_uptime(secs: u64) -> String {
    if secs < 60 {
        format!("{secs}s")
    } else if secs < 3_600 {
        format!("{}m", secs / 60)
    } else {
        format!("{}h {}m", secs / 3_600, (secs % 3_600) / 60)
    }
}
