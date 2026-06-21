//! Routines tab: list, create, edit, trigger, logs, and delete agent-driven scheduled jobs.
//!
//! Mirrors the cron-jobs UI but targets the `/routines` API. A routine launches an AI agent
//! (claude, codex, …) on a schedule instead of running a handler script.

use std::rc::Rc;

use chrono::{Datelike, Duration, Local, NaiveDate, TimeZone};
use gloo_net::http::Request;
use serde::{Deserialize, Serialize};
use wasm_bindgen_futures::spawn_local;
use web_sys::{HtmlInputElement, HtmlSelectElement};
use yew::prelude::*;

use crate::{describe_cron_live, parse_cron, reltime, ToastKind};

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
    pub last_manual_trigger_at: Option<u64>,
    /// Last time the routine fired on its cron schedule (the scheduled-fire mirror of
    /// `last_manual_trigger_at`). Absent on the bare `Routine` returned by `/trigger`.
    #[serde(default)]
    pub last_scheduled_trigger_at: Option<u64>,
    /// Workbench retention (seconds) for finished runs; `None` falls back to the server default.
    #[serde(default)]
    pub ttl_secs: Option<u64>,
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
    /// Workbench retention (seconds); `None` lets the server apply its default.
    pub ttl_secs: Option<u64>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ttl_secs: Option<u64>,
}

// ─── API layer ────────────────────────────────────────────────────────────────

async fn api_list() -> Result<Vec<Routine>, String> {
    Request::get("/api/v1/routines")
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json::<Vec<Routine>>()
        .await
        .map_err(|e| e.to_string())
}

async fn api_agents() -> Result<Vec<String>, String> {
    Request::get("/api/v1/agents")
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json::<Vec<String>>()
        .await
        .map_err(|e| e.to_string())
}

async fn api_create(req: &CreateRoutineRequest) -> Result<Routine, String> {
    let resp = Request::post("/api/v1/routines")
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
    let resp = Request::patch(&format!("/api/v1/routines/{id}"))
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
    let resp = Request::delete(&format!("/api/v1/routines/{id}"))
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
    let resp = Request::post(&format!("/api/v1/routines/{id}/trigger"))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.ok() {
        return Err(format!("HTTP {}", resp.status()));
    }
    resp.json::<Routine>().await.map_err(|e| e.to_string())
}

async fn api_cleanup() -> Result<usize, String> {
    let resp = Request::post("/api/v1/routines/cleanup")
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
    let resp = Request::get(&format!("/api/v1/routines/{id}/logs"))
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

/// How the list page presents routines: a table, or a month calendar of upcoming fire times.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum RView {
    #[default]
    Table,
    Calendar,
}

/// Field the routine table is sorted by.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RSort {
    #[default]
    Created,
    Updated,
    Title,
    Repository,
}

impl RSort {
    /// Parse the value of the sort `<select>`.
    fn from_str(s: &str) -> Self {
        match s {
            "updated" => RSort::Updated,
            "title" => RSort::Title,
            "repository" => RSort::Repository,
            _ => RSort::Created,
        }
    }

    /// `<option>` value for this sort field.
    fn as_str(self) -> &'static str {
        match self {
            RSort::Created => "created",
            RSort::Updated => "updated",
            RSort::Title => "title",
            RSort::Repository => "repository",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RState {
    pub routines: Vec<Routine>,
    pub loading: bool,
    pub page: RPage,
    pub modal: RModal,
    pub view: RView,
    /// Case-insensitive repository-URL substring filter for the table.
    pub repo_filter: String,
    /// Field the table is sorted by.
    pub sort: RSort,
    /// `true` sorts descending (newest / Z→A first).
    pub sort_desc: bool,
}

impl Default for RState {
    fn default() -> Self {
        Self {
            routines: vec![],
            loading: true,
            page: RPage::List,
            modal: RModal::None,
            view: RView::default(),
            repo_filter: String::new(),
            sort: RSort::default(),
            sort_desc: false,
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
    SetView(RView),
    SetRepoFilter(String),
    SetSort(RSort),
    ToggleSortDir,
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
            RAction::SetView(view) => s.view = view,
            RAction::SetRepoFilter(f) => s.repo_filter = f,
            RAction::SetSort(sort) => s.sort = sort,
            RAction::ToggleSortDir => s.sort_desc = !s.sort_desc,
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
    let on_set_view = {
        let state = state.clone();
        Callback::from(move |view: RView| state.dispatch(RAction::SetView(view)))
    };
    let on_repo_filter = {
        let state = state.clone();
        Callback::from(move |f: String| state.dispatch(RAction::SetRepoFilter(f)))
    };
    let on_set_sort = {
        let state = state.clone();
        Callback::from(move |sort: RSort| state.dispatch(RAction::SetSort(sort)))
    };
    let on_toggle_sort_dir = {
        let state = state.clone();
        Callback::from(move |_: ()| state.dispatch(RAction::ToggleSortDir))
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
                        ttl_secs: req.ttl_secs,
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
    let view = state.view;
    let repo_filter = state.repo_filter.clone();
    let sort = state.sort;
    let sort_desc = state.sort_desc;

    // Repository filter + sort applied client-side; mirrors the `repository`/`sort`/`order`
    // query params the `/routines` API accepts.
    let visible = {
        let needle = repo_filter.trim().to_lowercase();
        let mut v: Vec<Routine> = routines
            .iter()
            .filter(|r| {
                needle.is_empty()
                    || r.repositories
                        .iter()
                        .any(|repo| repo.repository.to_lowercase().contains(&needle))
            })
            .cloned()
            .collect();
        match sort {
            RSort::Created => v.sort_by_key(|r| r.created_at),
            RSort::Updated => v.sort_by_key(|r| r.updated_at),
            RSort::Title => v.sort_by_key(|r| r.title.to_lowercase()),
            // Routines with a repository sort before those without, then by primary URL.
            RSort::Repository => v.sort_by_key(|r| match r.repositories.first() {
                Some(repo) => (false, repo.repository.to_lowercase()),
                None => (true, String::new()),
            }),
        }
        if sort_desc {
            v.reverse();
        }
        v
    };

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
                                <div class="section-acts">
                                    <ViewToggle view={view} on_set_view={on_set_view} />
                                    <button class="btn btn-ghost btn-sm" onclick={on_cleanup}
                                        title="Reap finished, expired run workbenches now">{"CLEANUP NOW"}</button>
                                    <button class="btn btn-primary btn-sm" onclick={on_new}>{"+ NEW ROUTINE"}</button>
                                </div>
                            </div>
                            <FilterSortBar
                                repo_filter={repo_filter}
                                sort={sort}
                                sort_desc={sort_desc}
                                on_repo_filter={on_repo_filter}
                                on_set_sort={on_set_sort}
                                on_toggle_sort_dir={on_toggle_sort_dir}
                            />
                            {
                                match view {
                                    RView::Table => html! {
                                        <RoutineTable
                                            routines={visible}
                                            loading={loading}
                                            on_edit={on_edit}
                                            on_delete={on_ask_delete}
                                            on_toggle={on_toggle}
                                            on_trigger={on_trigger}
                                            on_logs={on_logs}
                                        />
                                    },
                                    RView::Calendar => html! {
                                        <RoutineCalendar routines={visible} loading={loading} />
                                    },
                                }
                            }
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

// ─── View toggle ──────────────────────────────────────────────────────────────

#[derive(Properties, PartialEq)]
pub struct ViewToggleProps {
    pub view: RView,
    pub on_set_view: Callback<RView>,
}

#[function_component(ViewToggle)]
pub fn view_toggle(props: &ViewToggleProps) -> Html {
    let mk = |view: RView, label: &'static str| {
        let cb = props.on_set_view.clone();
        let cls = if props.view == view {
            "view-btn active"
        } else {
            "view-btn"
        };
        html! {
            <button class={cls} onclick={Callback::from(move |_: MouseEvent| cb.emit(view))}>
                { label }
            </button>
        }
    };
    html! {
        <div class="view-toggle">
            { mk(RView::Table, "LIST") }
            { mk(RView::Calendar, "CALENDAR") }
        </div>
    }
}

// ─── Filter & sort bar ────────────────────────────────────────────────────────

#[derive(Properties, PartialEq)]
pub struct FilterSortBarProps {
    pub repo_filter: String,
    pub sort: RSort,
    pub sort_desc: bool,
    pub on_repo_filter: Callback<String>,
    pub on_set_sort: Callback<RSort>,
    pub on_toggle_sort_dir: Callback<()>,
}

/// Repository filter input plus a sort-field dropdown and direction toggle for the routine table.
#[function_component(FilterSortBar)]
pub fn filter_sort_bar(props: &FilterSortBarProps) -> Html {
    let on_input = {
        let cb = props.on_repo_filter.clone();
        Callback::from(move |e: InputEvent| {
            let input: HtmlInputElement = e.target_unchecked_into();
            cb.emit(input.value());
        })
    };
    let on_clear = {
        let cb = props.on_repo_filter.clone();
        Callback::from(move |_: MouseEvent| cb.emit(String::new()))
    };
    let on_sort_change = {
        let cb = props.on_set_sort.clone();
        Callback::from(move |e: Event| {
            let select: HtmlSelectElement = e.target_unchecked_into();
            cb.emit(RSort::from_str(&select.value()));
        })
    };
    let on_dir = {
        let cb = props.on_toggle_sort_dir.clone();
        Callback::from(move |_: MouseEvent| cb.emit(()))
    };
    let dir_label = if props.sort_desc {
        "↓ DESC"
    } else {
        "↑ ASC"
    };
    let current = props.sort.as_str();

    html! {
        <div class="filter-bar">
            <div class="filter-field">
                <input
                    type="text"
                    class="filter-input"
                    placeholder="Filter by repository…"
                    value={props.repo_filter.clone()}
                    oninput={on_input}
                />
                {
                    if props.repo_filter.is_empty() {
                        html! {}
                    } else {
                        html! {
                            <button class="btn btn-ghost btn-sm" onclick={on_clear}
                                title="Clear repository filter">{"✕"}</button>
                        }
                    }
                }
            </div>
            <div class="filter-field">
                <span class="filter-label">{"SORT"}</span>
                <select class="filter-select" onchange={on_sort_change}>
                    <option value="created" selected={current == "created"}>{"Created"}</option>
                    <option value="updated" selected={current == "updated"}>{"Updated"}</option>
                    <option value="title" selected={current == "title"}>{"Title"}</option>
                    <option value="repository" selected={current == "repository"}>{"Repository"}</option>
                </select>
                <button class="btn btn-ghost btn-sm" onclick={on_dir}
                    title="Toggle sort direction">{dir_label}</button>
            </div>
        </div>
    }
}

// ─── Calendar ─────────────────────────────────────────────────────────────────

const WEEKDAYS: [&str; 7] = ["SUN", "MON", "TUE", "WED", "THU", "FRI", "SAT"];
const MONTHS: [&str; 12] = [
    "JANUARY",
    "FEBRUARY",
    "MARCH",
    "APRIL",
    "MAY",
    "JUNE",
    "JULY",
    "AUGUST",
    "SEPTEMBER",
    "OCTOBER",
    "NOVEMBER",
    "DECEMBER",
];
/// Cells in the month grid: 6 weeks × 7 days, always, so the layout never reflows.
const GRID_CELLS: usize = 42;
/// Upper bound on fire-time iterations per routine across the visible grid. A `@hourly`
/// routine fires ~1008 times across 42 days; this leaves headroom while bounding cost.
const MAX_OCCURRENCES: usize = 4000;

/// First day of the month `offset` months away from the month containing `today`.
fn month_start(today: NaiveDate, offset: i32) -> NaiveDate {
    let total = today.year() * 12 + today.month0() as i32 + offset;
    let year = total.div_euclid(12);
    let month0 = total.rem_euclid(12) as u32;
    NaiveDate::from_ymd_opt(year, month0 + 1, 1).unwrap_or(today)
}

/// One routine's fire counts per grid cell over `[grid_start, grid_start + 42 days)`.
fn occurrences_per_day(schedule: &str, grid_start: NaiveDate) -> Option<[u32; GRID_CELLS]> {
    let cron = parse_cron(schedule)?;
    let start_naive = grid_start.and_hms_opt(0, 0, 0)?;
    // Step back one second so an occurrence exactly at midnight on the first cell counts.
    let start = Local
        .from_local_datetime(&start_naive)
        .earliest()?
        .checked_sub_signed(Duration::seconds(1))?;
    let mut counts = [0u32; GRID_CELLS];
    for dt in cron.iter_after(start).take(MAX_OCCURRENCES) {
        let day = (dt.date_naive() - grid_start).num_days();
        if day < 0 {
            continue;
        }
        if day as usize >= GRID_CELLS {
            break;
        }
        counts[day as usize] += 1;
    }
    Some(counts)
}

#[derive(Properties, PartialEq)]
pub struct CalendarProps {
    pub routines: Vec<Routine>,
    pub loading: bool,
}

#[function_component(RoutineCalendar)]
pub fn routine_calendar(props: &CalendarProps) -> Html {
    let offset = use_state(|| 0i32);

    let on_prev = {
        let offset = offset.clone();
        Callback::from(move |_: MouseEvent| offset.set(*offset - 1))
    };
    let on_next = {
        let offset = offset.clone();
        Callback::from(move |_: MouseEvent| offset.set(*offset + 1))
    };
    let on_today = {
        let offset = offset.clone();
        Callback::from(move |_: MouseEvent| offset.set(0))
    };

    if props.loading {
        return html! {
            <div class="table-wrap"><div class="empty"><div class="spinner"></div></div></div>
        };
    }

    let today = Local::now().date_naive();
    let first = month_start(today, *offset);
    let grid_start = first - Duration::days(first.weekday().num_days_from_sunday() as i64);

    // Accumulate per-cell chips in routine order: only enabled routines with a parseable schedule.
    let mut cells: Vec<Vec<(String, u32)>> = vec![Vec::new(); GRID_CELLS];
    let mut scheduled = 0usize;
    for r in props.routines.iter().filter(|r| r.enabled) {
        if let Some(counts) = occurrences_per_day(&r.schedule, grid_start) {
            scheduled += 1;
            for (i, &c) in counts.iter().enumerate() {
                if c > 0 {
                    cells[i].push((r.title.clone(), c));
                }
            }
        }
    }

    let month_label = format!("{} {}", MONTHS[first.month0() as usize], first.year());

    let body = if scheduled == 0 {
        html! {
            <div class="empty">
                <div class="empty-icon">{"🗓"}</div>
                <div class="empty-msg">{"NOTHING SCHEDULED"}</div>
                <div class="empty-sub">{"enabled routines with a valid schedule appear here"}</div>
            </div>
        }
    } else {
        html! {
            <>
                <div class="cal-weekdays">
                    { for WEEKDAYS.iter().map(|d| html! { <div class="cal-weekday">{*d}</div> }) }
                </div>
                <div class="cal-grid">
                    { for cells.iter().enumerate().map(|(i, hits)| {
                        let date = grid_start + Duration::days(i as i64);
                        let mut cls = String::from("cal-day");
                        if date.month() != first.month() {
                            cls.push_str(" other-month");
                        }
                        if date == today {
                            cls.push_str(" today");
                        }
                        html! {
                            <div class={cls}>
                                <div class="cal-daynum">{date.day()}</div>
                                <div class="cal-hits">
                                    { for hits.iter().take(4).map(|(title, count)| {
                                        let label = if *count > 1 {
                                            format!("{title} ×{count}")
                                        } else {
                                            title.clone()
                                        };
                                        html! { <div class="cal-chip" title={label.clone()}>{label}</div> }
                                    }) }
                                    if hits.len() > 4 {
                                        <div class="cal-more">{format!("+{} more", hits.len() - 4)}</div>
                                    }
                                </div>
                            </div>
                        }
                    }) }
                </div>
            </>
        }
    };

    html! {
        <div class="cal-wrap">
            <div class="cal-nav">
                <button class="btn-refresh" title="Previous month" onclick={on_prev}>{"‹"}</button>
                <div class="cal-month">{month_label}</div>
                <button class="btn-refresh" title="Next month" onclick={on_next}>{"›"}</button>
                <button class="btn btn-ghost btn-sm" onclick={on_today}>{"TODAY"}</button>
            </div>
            {body}
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
                        <th>{"TTL"}</th>
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

/// Most-recent fire across the scheduled and manual trigger timestamps, tagged with a source
/// icon (`⏱` scheduled, `↻` manual). Returns `None` only when the routine has never fired on
/// schedule *and* was never manually triggered — the UI renders that case as a "never fired"
/// badge so a dead schedule is visually distinct from a healthy one. Ties favor the scheduled
/// source (a schedule that just fired is the operationally interesting signal).
fn last_fired(scheduled: Option<u64>, manual: Option<u64>) -> Option<(&'static str, u64)> {
    match (scheduled, manual) {
        (Some(s), Some(m)) => Some(if s >= m { ("⏱", s) } else { ("↻", m) }),
        (Some(s), None) => Some(("⏱", s)),
        (None, Some(m)) => Some(("↻", m)),
        (None, None) => None,
    }
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

    // Fold the scheduled and manual fire timestamps into a single "last run" cell, showing
    // whichever is most recent with its source icon. A routine that has never fired either way
    // is flagged so dead schedules stand out from healthy ones (see #487, completing #155).
    let last_run = last_fired(r.last_scheduled_trigger_at, r.last_manual_trigger_at);

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
            <td><span class="cell-meta" title="workbench retention for finished runs">{ format_ttl(r.ttl_secs) }</span></td>
            <td>
                <label class="toggle">
                    <input type="checkbox" checked={r.enabled} onchange={on_toggle} />
                    <div class="toggle-track"></div>
                </label>
            </td>
            <td>
                <div class="cell-time">{updated}</div>
                {
                    match last_run {
                        Some((icon, ts)) => html! {
                            <div class="cell-triggered">{format!("{icon} {}", reltime(ts))}</div>
                        },
                        None => html! {
                            <div class="cell-triggered never" title="this routine has never fired on schedule and was never triggered manually">{"never fired"}</div>
                        },
                    }
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

/// Parse a TTL textarea value into seconds. Blank/whitespace → `None` (use the server default);
/// a valid non-negative integer → `Some(secs)`; anything else → `None`.
fn parse_ttl(raw: &str) -> Option<u64> {
    let t = raw.trim();
    if t.is_empty() {
        None
    } else {
        t.parse::<u64>().ok()
    }
}

/// Render a routine's TTL for display: `None` shows the server default, otherwise a compact
/// duration (`7d`, `12h`, `30m`, `45s`).
fn format_ttl(ttl_secs: Option<u64>) -> String {
    match ttl_secs {
        None => "default".to_string(),
        Some(0) => "0s".to_string(),
        Some(s) if s % 86_400 == 0 => format!("{}d", s / 86_400),
        Some(s) if s % 3_600 == 0 => format!("{}h", s / 3_600),
        Some(s) if s % 60 == 0 => format!("{}m", s / 60),
        Some(s) => format!("{s}s"),
    }
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
    // Blank means "use the server default"; otherwise the workbench TTL in seconds.
    let ttl_raw = use_state(|| {
        editing
            .as_ref()
            .and_then(|r| r.ttl_secs)
            .map(|s| s.to_string())
            .unwrap_or_default()
    });
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
    let on_ttl = {
        let ttl_raw = ttl_raw.clone();
        Callback::from(move |e: InputEvent| {
            let i: HtmlInputElement = e.target_unchecked_into();
            ttl_raw.set(i.value());
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
        let ttl_raw = ttl_raw.clone();
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
                ttl_secs: parse_ttl(&ttl_raw),
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
            <div class="form-group">
                <label class="form-label">
                    {"WORKBENCH TTL "}
                    <span style="color:var(--text-ghost)">{"(seconds; blank = server default)"}</span>
                </label>
                <input class="form-input" type="number" min="0" placeholder="604800"
                    value={(*ttl_raw).clone()} oninput={on_ttl} autocomplete="off" spellcheck="false" />
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
