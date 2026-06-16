//! Routines tab: list, create, edit, trigger, logs, and delete agent-driven scheduled jobs.
//!
//! Mirrors the cron-jobs UI but targets the `/routines` API. A routine launches an AI agent
//! (claude, codex, …) on a schedule instead of running a handler script.

use std::rc::Rc;

use gloo_net::http::Request;
use serde::{Deserialize, Serialize};
use wasm_bindgen_futures::spawn_local;
use web_sys::{HtmlInputElement, HtmlSelectElement};
use yew::prelude::*;

use crate::{describe_cron_live, reltime, ToastKind};

/// Agents the daemon ships built-in configs for (see `src/routines/agents`). Keep in sync with
/// `DEFAULT_AGENT_CONFIGS`.
pub const AVAILABLE_AGENTS: &[&str] = &["claude", "codex"];

// ─── Types (mirror server API exactly) ────────────────────────────────────────

/// A git repository listed in a routine's prompt as context.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Repository {
    pub repository: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
}

/// A routine as returned by `GET /routines` (the flattened `RoutineResponse`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Routine {
    pub id: String,
    pub schedule: String,
    pub title: String,
    pub agent: String,
    pub prompt: String,
    #[serde(default)]
    pub repositories: Vec<Repository>,
    pub enabled: bool,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub created_at: u64,
    #[serde(default)]
    pub updated_at: u64,
    #[serde(default)]
    pub last_triggered_at: Option<u64>,
    // Derived (absent on the bare Routine returned by /trigger — default to safe values).
    #[serde(default)]
    pub agent_registered: bool,
    #[serde(default)]
    pub file_path: String,
    #[serde(default)]
    pub schedule_description: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CreateRoutineRequest {
    pub schedule: String,
    pub title: String,
    pub agent: String,
    pub prompt: String,
    pub repositories: Vec<Repository>,
    pub enabled: bool,
}

/// Result of `POST /routines/cleanup` (mirrors the server `CleanupResponse`).
#[derive(Debug, Clone, Deserialize)]
pub struct CleanupResponse {
    pub removed: usize,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct UpdateRoutineRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schedule: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repositories: Option<Vec<Repository>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
}

// ─── API layer ────────────────────────────────────────────────────────────────

async fn api_list() -> Result<Vec<Routine>, String> {
    Request::get("/routines")
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json::<Vec<Routine>>()
        .await
        .map_err(|e| e.to_string())
}

async fn api_agents() -> Result<Vec<String>, String> {
    Request::get("/agents")
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json::<Vec<String>>()
        .await
        .map_err(|e| e.to_string())
}

async fn api_create(req: &CreateRoutineRequest) -> Result<Routine, String> {
    let resp = Request::post("/routines")
        .json(req)
        .map_err(|e| e.to_string())?
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.ok() {
        return Err(format!("HTTP {}", resp.status()));
    }
    resp.json::<Routine>().await.map_err(|e| e.to_string())
}

async fn api_update(id: &str, req: &UpdateRoutineRequest) -> Result<Routine, String> {
    let resp = Request::patch(&format!("/routines/{id}"))
        .json(req)
        .map_err(|e| e.to_string())?
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.ok() {
        return Err(format!("HTTP {}", resp.status()));
    }
    resp.json::<Routine>().await.map_err(|e| e.to_string())
}

async fn api_delete(id: &str) -> Result<(), String> {
    let resp = Request::delete(&format!("/routines/{id}"))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if resp.ok() {
        Ok(())
    } else {
        Err(format!("HTTP {}", resp.status()))
    }
}

async fn api_trigger(id: &str) -> Result<Routine, String> {
    let resp = Request::post(&format!("/routines/{id}/trigger"))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.ok() {
        return Err(format!("HTTP {}", resp.status()));
    }
    resp.json::<Routine>().await.map_err(|e| e.to_string())
}

async fn api_cleanup() -> Result<usize, String> {
    let resp = Request::post("/routines/cleanup")
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.ok() {
        return Err(format!("HTTP {}", resp.status()));
    }
    resp.json::<CleanupResponse>()
        .await
        .map(|r| r.removed)
        .map_err(|e| e.to_string())
}

async fn api_logs(id: &str) -> Result<String, String> {
    let resp = Request::get(&format!("/routines/{id}/logs"))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.ok() {
        return Err(format!("HTTP {}", resp.status()));
    }
    resp.text().await.map_err(|e| e.to_string())
}

// ─── State ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Default)]
pub enum RPage {
    #[default]
    List,
    New,
    Logs(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum RModal {
    None,
    Edit(String),
    ConfirmDelete { id: String, title: String },
}

#[derive(Debug, Clone, PartialEq)]
pub struct RState {
    pub routines: Vec<Routine>,
    pub loading: bool,
    pub page: RPage,
    pub modal: RModal,
}

impl Default for RState {
    fn default() -> Self {
        Self {
            routines: vec![],
            loading: true,
            page: RPage::List,
            modal: RModal::None,
        }
    }
}

pub enum RAction {
    Loaded(Vec<Routine>),
    GoToNew,
    GoToList,
    GoToLogs(String),
    OpenEdit(String),
    OpenConfirmDelete { id: String, title: String },
    CloseModal,
    Upsert(Routine),
    Remove(String),
}

impl Reducible for RState {
    type Action = RAction;

    fn reduce(self: Rc<Self>, action: Self::Action) -> Rc<Self> {
        let mut s = (*self).clone();
        match action {
            RAction::Loaded(r) => {
                s.routines = r;
                s.loading = false;
            }
            RAction::GoToNew => s.page = RPage::New,
            RAction::GoToList => s.page = RPage::List,
            RAction::GoToLogs(id) => s.page = RPage::Logs(id),
            RAction::OpenEdit(id) => s.modal = RModal::Edit(id),
            RAction::OpenConfirmDelete { id, title } => {
                s.modal = RModal::ConfirmDelete { id, title }
            }
            RAction::CloseModal => s.modal = RModal::None,
            RAction::Upsert(routine) => {
                if let Some(i) = s.routines.iter().position(|x| x.id == routine.id) {
                    s.routines[i] = routine;
                } else {
                    s.routines.push(routine);
                }
            }
            RAction::Remove(id) => s.routines.retain(|x| x.id != id),
        }
        s.into()
    }
}

// ─── Page component ───────────────────────────────────────────────────────────

#[derive(Properties, PartialEq)]
pub struct RoutinesPageProps {
    pub on_toast: Callback<(String, ToastKind)>,
}

#[function_component(RoutinesPage)]
pub fn routines_page(props: &RoutinesPageProps) -> Html {
    let state = use_reducer(RState::default);
    let toast = props.on_toast.clone();

    let ok_toast = {
        let toast = toast.clone();
        move |msg: &str| toast.emit((msg.to_string(), ToastKind::Ok))
    };

    // Load on mount.
    {
        let state = state.clone();
        let toast = toast.clone();
        use_effect_with((), move |_| {
            spawn_local(async move {
                match api_list().await {
                    Ok(r) => state.dispatch(RAction::Loaded(r)),
                    Err(e) => toast.emit((format!("Failed to load routines: {e}"), ToastKind::Err)),
                }
            });
        });
    }

    let on_new = {
        let state = state.clone();
        Callback::from(move |_: MouseEvent| state.dispatch(RAction::GoToNew))
    };
    let on_cancel = {
        let state = state.clone();
        Callback::from(move |_: ()| state.dispatch(RAction::GoToList))
    };
    let on_close = {
        let state = state.clone();
        Callback::from(move |_: ()| state.dispatch(RAction::CloseModal))
    };
    let on_logs = {
        let state = state.clone();
        Callback::from(move |id: String| state.dispatch(RAction::GoToLogs(id)))
    };
    let on_back = {
        let state = state.clone();
        Callback::from(move |_: ()| state.dispatch(RAction::GoToList))
    };
    let on_edit = {
        let state = state.clone();
        Callback::from(move |id: String| state.dispatch(RAction::OpenEdit(id)))
    };
    let on_ask_delete = {
        let state = state.clone();
        Callback::from(move |(id, title): (String, String)| {
            state.dispatch(RAction::OpenConfirmDelete { id, title })
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
                        state.dispatch(RAction::Upsert(r));
                        state.dispatch(RAction::GoToList);
                        ok("Routine created");
                    }
                    Err(e) => toast.emit((format!("Create failed: {e}"), ToastKind::Err)),
                }
            })
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
                    Ok(n) => ok(&format!(
                        "Cleanup removed {n} workbench{}",
                        if n == 1 { "" } else { "es" }
                    )),
                    Err(e) => toast.emit((format!("Cleanup failed: {e}"), ToastKind::Err)),
                }
            })
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
                        state.dispatch(RAction::Upsert(r));
                        ok("Routine triggered");
                    }
                    Err(e) => toast.emit((format!("Trigger failed: {e}"), ToastKind::Err)),
                }
            })
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
                        state.dispatch(RAction::Upsert(r));
                        ok(if enabled {
                            "Routine enabled"
                        } else {
                            "Routine disabled"
                        });
                    }
                    Err(e) => toast.emit((format!("Toggle failed: {e}"), ToastKind::Err)),
                }
            })
        })
    };

    let current_modal = state.modal.clone();
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
                        prompt: Some(req.prompt),
                        repositories: Some(req.repositories),
                        enabled: Some(req.enabled),
                    };
                    match api_update(id, &upd).await {
                        Ok(r) => {
                            state.dispatch(RAction::Upsert(r));
                            state.dispatch(RAction::CloseModal);
                            ok("Routine updated");
                        }
                        Err(e) => toast.emit((format!("Update failed: {e}"), ToastKind::Err)),
                    }
                }
            })
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
            })
        })
    };

    let routines = state.routines.clone();
    let loading = state.loading;
    let page = state.page.clone();
    let modal = state.modal.clone();

    let edit_routine = match &modal {
        RModal::Edit(id) => routines.iter().find(|r| r.id == *id).cloned(),
        _ => None,
    };

    html! {
        <>
            {
                match page {
                    RPage::New => html! {
                        <RoutineForm editing={None} on_cancel={on_cancel} on_save={on_create} />
                    },
                    RPage::Logs(id) => {
                        let title = routines.iter()
                            .find(|r| r.id == id)
                            .map(|r| r.title.clone())
                            .unwrap_or_default();
                        html! { <RoutineLogs id={id} title={title} on_back={on_back} /> }
                    },
                    RPage::List => html! {
                        <main>
                            <RoutineStats routines={routines.clone()} />
                            <div class="section-hd">
                                <div class="section-label">{"SCHEDULED ROUTINES"}</div>
                                <div class="section-actions">
                                    <button class="btn btn-ghost btn-sm" onclick={on_cleanup}
                                        title="Reap finished, expired run workbenches now">{"CLEANUP NOW"}</button>
                                    <button class="btn btn-primary btn-sm" onclick={on_new}>{"+ NEW ROUTINE"}</button>
                                </div>
                            </div>
                            <RoutineTable
                                routines={routines}
                                loading={loading}
                                on_edit={on_edit}
                                on_delete={on_ask_delete}
                                on_toggle={on_toggle}
                                on_trigger={on_trigger}
                                on_logs={on_logs}
                            />
                        </main>
                    },
                }
            }
            {
                match &modal {
                    RModal::Edit(_) => html! {
                        <RoutineForm editing={edit_routine} on_cancel={on_close.clone()} on_save={on_save} />
                    },
                    RModal::ConfirmDelete { id, title } => html! {
                        <ConfirmDelete
                            id={id.clone()}
                            title={title.clone()}
                            on_cancel={on_close}
                            on_confirm={on_confirm_delete}
                        />
                    },
                    RModal::None => html! {},
                }
            }
        </>
    }
}

// ─── Stats ────────────────────────────────────────────────────────────────────

#[derive(Properties, PartialEq)]
pub struct StatsProps {
    pub routines: Vec<Routine>,
}

#[function_component(RoutineStats)]
pub fn routine_stats(props: &StatsProps) -> Html {
    let total = props.routines.len();
    let enabled = props.routines.iter().filter(|r| r.enabled).count();
    let disabled = total - enabled;
    let unreg = props
        .routines
        .iter()
        .filter(|r| !r.agent_registered)
        .count();

    html! {
        <div class="stats">
            <div class="stat-card all">
                <div class="stat-label">{"TOTAL"}</div>
                <div class="stat-val">{total}</div>
            </div>
            <div class="stat-card enabled">
                <div class="stat-label">{"ENABLED"}</div>
                <div class="stat-val c-accent">{enabled}</div>
            </div>
            <div class="stat-card disabled">
                <div class="stat-label">{"DISABLED"}</div>
                <div class="stat-val c-amber">{disabled}</div>
            </div>
            <div class="stat-card unreg">
                <div class="stat-label">{"UNREGISTERED AGENT"}</div>
                <div class="stat-val">{unreg}</div>
            </div>
        </div>
    }
}

// ─── Table ────────────────────────────────────────────────────────────────────

#[derive(Properties, PartialEq)]
pub struct TableProps {
    pub routines: Vec<Routine>,
    pub loading: bool,
    pub on_edit: Callback<String>,
    pub on_delete: Callback<(String, String)>,
    pub on_toggle: Callback<(String, bool)>,
    pub on_trigger: Callback<String>,
    pub on_logs: Callback<String>,
}

#[function_component(RoutineTable)]
pub fn routine_table(props: &TableProps) -> Html {
    if props.loading {
        return html! {
            <div class="table-wrap"><div class="empty"><div class="spinner"></div></div></div>
        };
    }
    if props.routines.is_empty() {
        return html! {
            <div class="table-wrap">
                <div class="empty">
                    <div class="empty-icon">{"⧗"}</div>
                    <div class="empty-msg">{"NO ROUTINES SCHEDULED"}</div>
                    <div class="empty-sub">{"press + NEW ROUTINE to create one"}</div>
                </div>
            </div>
        };
    }

    html! {
        <div class="table-wrap">
            <table>
                <thead>
                    <tr>
                        <th>{"TITLE"}</th>
                        <th>{"SCHEDULE"}</th>
                        <th>{"AGENT"}</th>
                        <th>{"REPOS"}</th>
                        <th>{"ENABLED"}</th>
                        <th>{"UPDATED"}</th>
                        <th></th>
                    </tr>
                </thead>
                <tbody>
                    { for props.routines.iter().map(|r| html! {
                        <RoutineRow
                            key={r.id.clone()}
                            routine={r.clone()}
                            on_edit={props.on_edit.clone()}
                            on_delete={props.on_delete.clone()}
                            on_toggle={props.on_toggle.clone()}
                            on_trigger={props.on_trigger.clone()}
                            on_logs={props.on_logs.clone()}
                        />
                    }) }
                </tbody>
            </table>
        </div>
    }
}

#[derive(Properties, PartialEq)]
pub struct RowProps {
    pub routine: Routine,
    pub on_edit: Callback<String>,
    pub on_delete: Callback<(String, String)>,
    pub on_toggle: Callback<(String, bool)>,
    pub on_trigger: Callback<String>,
    pub on_logs: Callback<String>,
}

#[function_component(RoutineRow)]
pub fn routine_row(props: &RowProps) -> Html {
    let r = &props.routine;
    let cron_text = r.schedule_description.as_deref().unwrap_or("—").to_string();
    let updated = reltime(r.updated_at);
    let repos = r.repositories.len();

    let on_edit = {
        let cb = props.on_edit.clone();
        let id = r.id.clone();
        Callback::from(move |_: MouseEvent| cb.emit(id.clone()))
    };
    let on_delete = {
        let cb = props.on_delete.clone();
        let id = r.id.clone();
        let title = r.title.clone();
        Callback::from(move |_: MouseEvent| cb.emit((id.clone(), title.clone())))
    };
    let on_toggle = {
        let cb = props.on_toggle.clone();
        let id = r.id.clone();
        let enabled = r.enabled;
        Callback::from(move |_: Event| cb.emit((id.clone(), !enabled)))
    };
    let on_trigger = {
        let cb = props.on_trigger.clone();
        let id = r.id.clone();
        Callback::from(move |_: MouseEvent| cb.emit(id.clone()))
    };
    let on_logs = {
        let cb = props.on_logs.clone();
        let id = r.id.clone();
        Callback::from(move |_: MouseEvent| cb.emit(id.clone()))
    };

    let last_run = r
        .last_triggered_at
        .map(|t| format!("↻ {}", reltime(t)))
        .unwrap_or_default();

    let agent_dot = if r.agent_registered {
        "handler-dot ok"
    } else {
        "handler-dot warn"
    };
    let agent_title = if r.agent_registered {
        "agent registered"
    } else {
        "agent config missing"
    };

    html! {
        <tr>
            <td>
                <div class="cell-schedule" title={r.id.clone()}>{&r.title}</div>
            </td>
            <td>
                <div class="cell-schedule">{&r.schedule}</div>
                <div class="cell-schedule-human">{cron_text}</div>
            </td>
            <td>
                <span class="cell-handler" title={agent_title}>
                    <span class={agent_dot}></span>
                    {&r.agent}
                </span>
            </td>
            <td><span class="cell-meta">{ if repos == 0 { "—".to_string() } else { format!("{repos}") } }</span></td>
            <td>
                <label class="toggle">
                    <input type="checkbox" checked={r.enabled} onchange={on_toggle} />
                    <div class="toggle-track"></div>
                </label>
            </td>
            <td>
                <div class="cell-time">{updated}</div>
                if !last_run.is_empty() {
                    <div class="cell-triggered">{last_run}</div>
                }
            </td>
            <td>
                <div class="row-actions">
                    <button class="act-btn run" title="Run now" onclick={on_trigger}>{"▶"}</button>
                    <button class="act-btn logs" onclick={on_logs}>{"LOGS"}</button>
                    <button class="act-btn edit" onclick={on_edit}>{"EDIT"}</button>
                    <button class="act-btn del" onclick={on_delete}>{"✕"}</button>
                </div>
            </td>
        </tr>
    }
}

// ─── Form (create page + edit modal) ──────────────────────────────────────────

#[derive(Properties, PartialEq)]
pub struct FormProps {
    pub editing: Option<Routine>,
    pub on_cancel: Callback<()>,
    pub on_save: Callback<CreateRoutineRequest>,
}

/// Serialize repositories as one `url [branch]` line each for the textarea.
fn repos_to_text(repos: &[Repository]) -> String {
    repos
        .iter()
        .map(|r| match &r.branch {
            Some(b) if !b.is_empty() => format!("{} {}", r.repository, b),
            _ => r.repository.clone(),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Parse a textarea of `url [branch]` lines back into repositories.
fn text_to_repos(text: &str) -> Vec<Repository> {
    text.lines()
        .filter_map(|line| {
            let mut it = line.split_whitespace();
            let repository = it.next()?.to_string();
            let branch = it.next().map(|s| s.to_string());
            Some(Repository { repository, branch })
        })
        .collect()
}

#[function_component(RoutineForm)]
pub fn routine_form(props: &FormProps) -> Html {
    let editing = props.editing.clone();
    let is_edit = editing.is_some();

    let title = use_state(|| {
        editing
            .as_ref()
            .map(|r| r.title.clone())
            .unwrap_or_default()
    });
    let schedule = use_state(|| {
        editing
            .as_ref()
            .map(|r| r.schedule.clone())
            .unwrap_or_default()
    });
    let agent = use_state(|| {
        editing
            .as_ref()
            .map(|r| r.agent.clone())
            .unwrap_or_else(|| "claude".to_string())
    });
    // Agent options fetched from `GET /agents`; seed with the built-in list so the select is never
    // empty before the request resolves or if it fails.
    let agents = use_state(|| {
        AVAILABLE_AGENTS
            .iter()
            .map(|s| s.to_string())
            .collect::<Vec<_>>()
    });
    {
        let agents = agents.clone();
        use_effect_with((), move |_| {
            spawn_local(async move {
                if let Ok(list) = api_agents().await {
                    if !list.is_empty() {
                        agents.set(list);
                    }
                }
            });
            || ()
        });
    }
    let prompt = use_state(|| {
        editing
            .as_ref()
            .map(|r| r.prompt.clone())
            .unwrap_or_default()
    });
    let repos_raw = use_state(|| {
        editing
            .as_ref()
            .map(|r| repos_to_text(&r.repositories))
            .unwrap_or_default()
    });
    let enabled = use_state(|| editing.as_ref().map(|r| r.enabled).unwrap_or(true));
    let saving = use_state(|| false);

    let (cron_ok, cron_text) = describe_cron_live(&schedule);

    let on_title = {
        let title = title.clone();
        Callback::from(move |e: InputEvent| {
            let i: HtmlInputElement = e.target_unchecked_into();
            title.set(i.value());
        })
    };
    let on_schedule = {
        let schedule = schedule.clone();
        Callback::from(move |e: InputEvent| {
            let i: HtmlInputElement = e.target_unchecked_into();
            schedule.set(i.value());
        })
    };
    let on_agent = {
        let agent = agent.clone();
        Callback::from(move |e: Event| {
            let s: HtmlSelectElement = e.target_unchecked_into();
            agent.set(s.value());
        })
    };
    let on_prompt = {
        let prompt = prompt.clone();
        Callback::from(move |e: InputEvent| {
            let i: HtmlInputElement = e.target_unchecked_into();
            prompt.set(i.value());
        })
    };
    let on_repos = {
        let repos_raw = repos_raw.clone();
        Callback::from(move |e: InputEvent| {
            let i: HtmlInputElement = e.target_unchecked_into();
            repos_raw.set(i.value());
        })
    };
    let on_enabled = {
        let enabled = enabled.clone();
        Callback::from(move |e: Event| {
            let i: HtmlInputElement = e.target_unchecked_into();
            enabled.set(i.checked());
        })
    };

    let set_preset = |val: &'static str| {
        let schedule = schedule.clone();
        Callback::from(move |_: MouseEvent| schedule.set(val.to_string()))
    };

    let on_cancel_click = {
        let cb = props.on_cancel.clone();
        Callback::from(move |_: MouseEvent| cb.emit(()))
    };

    let can_save = !title.trim().is_empty()
        && !schedule.trim().is_empty()
        && !agent.trim().is_empty()
        && !prompt.trim().is_empty();

    let on_save_click = {
        let title = title.clone();
        let schedule = schedule.clone();
        let agent = agent.clone();
        let prompt = prompt.clone();
        let repos_raw = repos_raw.clone();
        let enabled = enabled.clone();
        let saving = saving.clone();
        let cb = props.on_save.clone();
        Callback::from(move |_: MouseEvent| {
            if !can_save {
                return;
            }
            saving.set(true);
            cb.emit(CreateRoutineRequest {
                schedule: (*schedule).clone(),
                title: (*title).clone(),
                agent: (*agent).clone(),
                prompt: (*prompt).clone(),
                repositories: text_to_repos(&repos_raw),
                enabled: *enabled,
            });
        })
    };

    let preview_class = if schedule.is_empty() {
        "cron-preview"
    } else if cron_ok {
        "cron-preview ok"
    } else {
        "cron-preview bad"
    };

    let submit_label = if *saving {
        "…"
    } else if is_edit {
        "SAVE CHANGES"
    } else {
        "CREATE ROUTINE"
    };

    let body = html! {
        <div class="modal-body">
            <div class="form-group">
                <label class="form-label">{"TITLE "}<span class="form-required">{"*"}</span></label>
                <input class="form-input" type="text" placeholder="nightly triage"
                    value={(*title).clone()} oninput={on_title} autocomplete="off" spellcheck="false" />
            </div>
            <div class="form-group">
                <label class="form-label">{"SCHEDULE "}<span class="form-required">{"*"}</span></label>
                <input class="form-input" type="text" placeholder="sec min hour dom month dow year"
                    value={(*schedule).clone()} oninput={on_schedule} autocomplete="off" spellcheck="false" />
                <div class="cron-presets">
                    { for [
                        ("@daily", "@daily"), ("@hourly", "@hourly"),
                        ("@weekly", "@weekly"), ("@monthly", "@monthly"),
                        ("0 0 9 * * 1-5 *", "weekdays 9am"),
                        ("0 0 * * * * *", "every hour"),
                    ].iter().map(|(val, label)| html! {
                        <button class="preset-btn" onclick={set_preset(val)}>{*label}</button>
                    }) }
                </div>
                <div class={preview_class}>{cron_text}</div>
            </div>
            <div class="form-group">
                <label class="form-label">{"AGENT "}<span class="form-required">{"*"}</span></label>
                <select class="form-input" onchange={on_agent}>
                    { for agents.iter().map(|name| html! {
                        <option value={name.clone()} selected={*agent == *name}>{name.clone()}</option>
                    }) }
                </select>
            </div>
            <div class="form-group">
                <label class="form-label">{"PROMPT "}<span class="form-required">{"*"}</span></label>
                <textarea class="form-input" placeholder="Review open PRs and summarize…"
                    value={(*prompt).clone()} oninput={on_prompt} />
            </div>
            <div class="form-group">
                <label class="form-label">
                    {"REPOSITORIES "}
                    <span style="color:var(--text-ghost)">{"(one url [branch] per line)"}</span>
                </label>
                <textarea class="form-input" placeholder={"https://github.com/org/repo main"}
                    value={(*repos_raw).clone()} oninput={on_repos} />
            </div>
            <div class="form-group" style="margin-bottom:0">
                <div class="toggle-row">
                    <span class="toggle-row-label">{"ENABLED"}</span>
                    <label class="toggle">
                        <input type="checkbox" checked={*enabled} onchange={on_enabled} />
                        <div class="toggle-track"></div>
                    </label>
                </div>
            </div>
        </div>
    };

    let footer = html! {
        <div class="modal-ft">
            <button class="btn btn-ghost btn-sm" onclick={on_cancel_click.clone()}>{"CANCEL"}</button>
            <button class="btn btn-primary btn-sm" onclick={on_save_click} disabled={*saving || !can_save}>
                { submit_label }
            </button>
        </div>
    };

    if is_edit {
        html! {
            <div class="overlay open">
                <div class="modal">
                    <div class="modal-hd">
                        <div class="modal-title">{"EDIT ROUTINE"}</div>
                        <button class="modal-x" onclick={on_cancel_click}>{"✕"}</button>
                    </div>
                    {body}
                    {footer}
                </div>
            </div>
        }
    } else {
        html! {
            <main class="create-page">
                <div class="page-hd">
                    <button class="btn btn-ghost btn-sm" onclick={on_cancel_click}>{"← BACK"}</button>
                    <div class="page-title">{"NEW ROUTINE"}</div>
                </div>
                <div class="page-card">
                    {body}
                    {footer}
                </div>
            </main>
        }
    }
}

// ─── Confirm delete ───────────────────────────────────────────────────────────

#[derive(Properties, PartialEq)]
pub struct ConfirmProps {
    pub id: String,
    pub title: String,
    pub on_cancel: Callback<()>,
    pub on_confirm: Callback<String>,
}

#[function_component(ConfirmDelete)]
pub fn confirm_delete(props: &ConfirmProps) -> Html {
    let on_cancel = {
        let cb = props.on_cancel.clone();
        Callback::from(move |_: MouseEvent| cb.emit(()))
    };
    let on_confirm = {
        let cb = props.on_confirm.clone();
        let id = props.id.clone();
        Callback::from(move |_: MouseEvent| cb.emit(id.clone()))
    };

    html! {
        <div class="overlay open">
            <div class="confirm-dialog">
                <div class="confirm-title">{"⚠ DELETE ROUTINE"}</div>
                <div class="confirm-msg">
                    { format!("Delete the routine \"{}\"? This cannot be undone.", props.title) }
                </div>
                <div class="confirm-acts">
                    <button class="btn btn-ghost btn-sm" onclick={on_cancel}>{"CANCEL"}</button>
                    <button class="btn btn-danger btn-sm" onclick={on_confirm}>{"DELETE"}</button>
                </div>
            </div>
        </div>
    }
}

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

    let load = {
        let id = props.id.clone();
        let content = content.clone();
        let loading = loading.clone();
        let err = err.clone();
        move || {
            let id = id.clone();
            let content = content.clone();
            let loading = loading.clone();
            let err = err.clone();
            loading.set(true);
            spawn_local(async move {
                match api_logs(&id).await {
                    Ok(text) => {
                        content.set(Some(text));
                        err.set(None);
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
    } else if let Some(e) = (*err).clone() {
        html! { <div class="logs-error">{format!("Error: {e}")}</div> }
    } else if let Some(text) = (*content).clone() {
        if text.is_empty() {
            html! { <div class="logs-empty">{"— no logs yet —"}</div> }
        } else {
            html! { <pre class="logs-content">{text}</pre> }
        }
    } else {
        html! {}
    };

    html! {
        <main class="logs-page">
            <div class="page-hd">
                <button class="btn btn-ghost btn-sm" onclick={on_back}>{"← BACK"}</button>
                <div class="page-title">{format!("LOGS / {}", props.title)}</div>
                <button class="btn-refresh" title="Refresh" onclick={on_refresh}>{"↻"}</button>
            </div>
            <div class="logs-wrap">
                {body}
            </div>
        </main>
    }
}
