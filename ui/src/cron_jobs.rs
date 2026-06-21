//! Cron-jobs page: list, create, edit, trigger, logs, and delete schedule-driven handler jobs.
//!
//! Self-contained like [`crate::routines::RoutinesPage`]: owns its own reducer state and talks to
//! the `/cron-jobs` API. Toasts bubble up to the shell via the `on_toast` callback.

use std::collections::HashSet;
use std::rc::Rc;

use gloo_net::http::Request;
use serde::{Deserialize, Serialize};
use serde_json::Value as Json;
use wasm_bindgen_futures::spawn_local;
use web_sys::HtmlInputElement;
use yew::prelude::*;

use crate::day_timeline::{DayTimeline, TimelineItem};
use crate::machines::MachinesPicker;
use crate::{describe_cron_live, reltime, ToastKind};

// ─── Types (mirror server API exactly) ────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CronJob {
    pub id: String,
    pub schedule: String,
    pub handler: String,
    pub metadata: Json,
    /// Machines this job runs on. An empty list runs nowhere (dormant until assigned).
    #[serde(default)]
    pub machines: Vec<String>,
    pub enabled: bool,
    pub created_at: u64,
    pub updated_at: u64,
    #[serde(default)]
    pub last_manual_trigger_at: Option<u64>,
    /// Human-readable schedule description supplied by the server (e.g. "At 09:30, Monday through Friday").
    #[serde(default)]
    pub schedule_description: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CreateRequest {
    pub schedule: String,
    pub handler: String,
    pub metadata: Json,
    /// Machines to run this job on (empty = runs nowhere until assigned).
    pub machines: Vec<String>,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct UpdateRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schedule: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub handler: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Json>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub machines: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
}

// ─── API layer ────────────────────────────────────────────────────────────────

async fn api_list() -> Result<Vec<CronJob>, String> {
    Request::get("/api/v1/cron-jobs")
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json::<Vec<CronJob>>()
        .await
        .map_err(|e| e.to_string())
}

async fn api_create(req: &CreateRequest) -> Result<CronJob, String> {
    let resp = Request::post("/api/v1/cron-jobs")
        .json(req)
        .map_err(|e| e.to_string())?
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.ok() {
        return Err(format!("HTTP {}", resp.status()));
    }
    resp.json::<CronJob>().await.map_err(|e| e.to_string())
}

async fn api_update(id: &str, req: &UpdateRequest) -> Result<CronJob, String> {
    let resp = Request::put(&format!("/api/v1/cron-jobs/{id}"))
        .json(req)
        .map_err(|e| e.to_string())?
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.ok() {
        return Err(format!("HTTP {}", resp.status()));
    }
    resp.json::<CronJob>().await.map_err(|e| e.to_string())
}

async fn api_delete(id: &str) -> Result<(), String> {
    let resp = Request::delete(&format!("/api/v1/cron-jobs/{id}"))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if resp.status() == 204 || resp.ok() {
        Ok(())
    } else {
        Err(format!("HTTP {}", resp.status()))
    }
}

async fn api_trigger(id: &str) -> Result<CronJob, String> {
    let resp = Request::post(&format!("/api/v1/cron-jobs/{id}/trigger"))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.ok() {
        return Err(format!("HTTP {}", resp.status()));
    }
    resp.json::<CronJob>().await.map_err(|e| e.to_string())
}

async fn api_logs(id: &str) -> Result<String, String> {
    let resp = Request::get(&format!("/api/v1/cron-jobs/{id}/logs"))
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
pub enum CPage {
    #[default]
    List,
    New,
    Logs(String),
}

/// How the list page presents jobs: a table, or a scrollable single-day timeline.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum CView {
    #[default]
    Table,
    Day,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CModal {
    None,
    Edit(String),
    ConfirmDelete { id: String, handler: String },
    ConfirmBulkDelete { count: usize },
}

/// How a row-selection click should mutate the selection set, derived from the
/// modifier keys held during the click.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SelectKind {
    /// Plain click — select only this item, clearing the rest.
    Only,
    /// Cmd/Ctrl+click — toggle this item in/out of the selection.
    Toggle,
    /// Shift+click — select the contiguous range from the anchor to this item.
    Range,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CState {
    pub jobs: Vec<CronJob>,
    pub loading: bool,
    pub page: CPage,
    pub modal: CModal,
    pub view: CView,
    /// IDs of currently selected jobs (multiselect).
    pub selected: HashSet<String>,
    /// Anchor row for `Shift`+click range selection.
    pub select_anchor: Option<String>,
}

impl Default for CState {
    fn default() -> Self {
        Self {
            jobs: vec![],
            loading: true,
            page: CPage::List,
            modal: CModal::None,
            view: CView::default(),
            selected: HashSet::new(),
            select_anchor: None,
        }
    }
}

pub enum CAction {
    Loaded(Vec<CronJob>),
    GoToNew,
    GoToList,
    GoToLogs(String),
    OpenEdit(String),
    OpenConfirmDelete {
        id: String,
        handler: String,
    },
    OpenConfirmBulkDelete,
    CloseModal,
    SetView(CView),
    Upsert(CronJob),
    Remove(String),
    RemoveMany(Vec<String>),
    /// Apply a selection click to the job with this id, interpreted per `kind`.
    SelectJob {
        id: String,
        kind: SelectKind,
    },
    SelectAll,
    ClearSelection,
}

impl Reducible for CState {
    type Action = CAction;

    fn reduce(self: Rc<Self>, action: Self::Action) -> Rc<Self> {
        let mut s = (*self).clone();
        match action {
            CAction::Loaded(jobs) => {
                // Drop selections for jobs that no longer exist after a reload.
                let ids: HashSet<&String> = jobs.iter().map(|j| &j.id).collect();
                s.selected.retain(|id| ids.contains(id));
                if let Some(a) = &s.select_anchor {
                    if !ids.contains(a) {
                        s.select_anchor = None;
                    }
                }
                s.jobs = jobs;
                s.loading = false;
            }
            CAction::GoToNew => s.page = CPage::New,
            CAction::GoToList => s.page = CPage::List,
            CAction::GoToLogs(id) => s.page = CPage::Logs(id),
            CAction::OpenEdit(id) => s.modal = CModal::Edit(id),
            CAction::OpenConfirmDelete { id, handler } => {
                s.modal = CModal::ConfirmDelete { id, handler }
            }
            CAction::OpenConfirmBulkDelete => {
                s.modal = CModal::ConfirmBulkDelete {
                    count: s.selected.len(),
                }
            }
            CAction::CloseModal => s.modal = CModal::None,
            CAction::SetView(view) => s.view = view,
            CAction::Upsert(job) => {
                if let Some(i) = s.jobs.iter().position(|j| j.id == job.id) {
                    s.jobs[i] = job;
                } else {
                    s.jobs.push(job);
                }
            }
            CAction::Remove(id) => {
                s.jobs.retain(|j| j.id != id);
                s.selected.remove(&id);
                if s.select_anchor.as_ref() == Some(&id) {
                    s.select_anchor = None;
                }
            }
            CAction::RemoveMany(ids) => {
                let drop: HashSet<&String> = ids.iter().collect();
                s.jobs.retain(|j| !drop.contains(&j.id));
                s.selected.retain(|id| !drop.contains(id));
                if let Some(a) = &s.select_anchor {
                    if drop.contains(a) {
                        s.select_anchor = None;
                    }
                }
            }
            CAction::SelectJob { id, kind } => match kind {
                SelectKind::Only => {
                    s.selected.clear();
                    s.selected.insert(id.clone());
                    s.select_anchor = Some(id);
                }
                SelectKind::Toggle => {
                    if !s.selected.remove(&id) {
                        s.selected.insert(id.clone());
                    }
                    s.select_anchor = Some(id);
                }
                SelectKind::Range => {
                    let anchor = s.select_anchor.clone().unwrap_or_else(|| id.clone());
                    let pos = |target: &str| s.jobs.iter().position(|j| j.id == target);
                    match (pos(&anchor), pos(&id)) {
                        (Some(a), Some(b)) => {
                            let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
                            for job in &s.jobs[lo..=hi] {
                                s.selected.insert(job.id.clone());
                            }
                        }
                        _ => {
                            s.selected.insert(id.clone());
                        }
                    }
                    // Anchor stays put so further Shift+clicks re-anchor from it.
                }
            },
            CAction::SelectAll => {
                s.selected = s.jobs.iter().map(|j| j.id.clone()).collect();
            }
            CAction::ClearSelection => {
                s.selected.clear();
                s.select_anchor = None;
            }
        }
        s.into()
    }
}

// ─── Page component ───────────────────────────────────────────────────────────

#[derive(Properties, PartialEq)]
pub struct CronJobsPageProps {
    pub on_toast: Callback<(String, ToastKind)>,
}

#[function_component(CronJobsPage)]
pub fn cron_jobs_page(props: &CronJobsPageProps) -> Html {
    let state = use_reducer(CState::default);
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
                    Ok(jobs) => state.dispatch(CAction::Loaded(jobs)),
                    Err(e) => toast.emit((format!("Failed to load jobs: {e}"), ToastKind::Err)),
                }
            });
        });
    }

    let on_new = {
        let state = state.clone();
        Callback::from(move |_: MouseEvent| state.dispatch(CAction::GoToNew))
    };
    let on_cancel = {
        let state = state.clone();
        Callback::from(move |_: ()| state.dispatch(CAction::GoToList))
    };
    let on_close = {
        let state = state.clone();
        Callback::from(move |_: ()| state.dispatch(CAction::CloseModal))
    };
    let on_logs = {
        let state = state.clone();
        Callback::from(move |id: String| state.dispatch(CAction::GoToLogs(id)))
    };
    let on_back = {
        let state = state.clone();
        Callback::from(move |_: ()| state.dispatch(CAction::GoToList))
    };
    let on_edit = {
        let state = state.clone();
        Callback::from(move |id: String| state.dispatch(CAction::OpenEdit(id)))
    };
    let on_ask_delete = {
        let state = state.clone();
        Callback::from(move |(id, handler): (String, String)| {
            state.dispatch(CAction::OpenConfirmDelete { id, handler })
        })
    };

    let on_create = {
        let state = state.clone();
        let toast = toast.clone();
        let ok = ok_toast.clone();
        Callback::from(move |req: CreateRequest| {
            let state = state.clone();
            let toast = toast.clone();
            let ok = ok.clone();
            spawn_local(async move {
                match api_create(&req).await {
                    Ok(job) => {
                        state.dispatch(CAction::Upsert(job));
                        state.dispatch(CAction::GoToList);
                        ok("Job created");
                    }
                    Err(e) => toast.emit((format!("Create failed: {e}"), ToastKind::Err)),
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
                    Ok(job) => {
                        state.dispatch(CAction::Upsert(job));
                        ok("Job triggered");
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
                let req = UpdateRequest {
                    enabled: Some(enabled),
                    ..Default::default()
                };
                match api_update(&id, &req).await {
                    Ok(job) => {
                        state.dispatch(CAction::Upsert(job));
                        ok(if enabled {
                            "Job enabled"
                        } else {
                            "Job disabled"
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
        Callback::from(move |req: CreateRequest| {
            let state = state.clone();
            let toast = toast.clone();
            let ok = ok.clone();
            let modal = current_modal.clone();
            spawn_local(async move {
                if let CModal::Edit(id) = &modal {
                    let upd = UpdateRequest {
                        schedule: Some(req.schedule),
                        handler: Some(req.handler),
                        metadata: Some(req.metadata),
                        machines: Some(req.machines),
                        enabled: Some(req.enabled),
                    };
                    match api_update(id, &upd).await {
                        Ok(job) => {
                            state.dispatch(CAction::Upsert(job));
                            state.dispatch(CAction::CloseModal);
                            ok("Job updated");
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
                        state.dispatch(CAction::Remove(id));
                        state.dispatch(CAction::CloseModal);
                        ok("Job deleted");
                    }
                    Err(e) => toast.emit((format!("Delete failed: {e}"), ToastKind::Err)),
                }
            })
        })
    };

    // ── Multiselect ──
    let on_select = {
        let state = state.clone();
        Callback::from(move |(id, kind): (String, SelectKind)| {
            state.dispatch(CAction::SelectJob { id, kind })
        })
    };

    // Header checkbox toggles between "all selected" and "none selected".
    let on_select_all = {
        let state = state.clone();
        Callback::from(move |_: ()| {
            if state.selected.len() == state.jobs.len() && !state.jobs.is_empty() {
                state.dispatch(CAction::ClearSelection);
            } else {
                state.dispatch(CAction::SelectAll);
            }
        })
    };

    let on_clear_selection = {
        let state = state.clone();
        Callback::from(move |_: ()| state.dispatch(CAction::ClearSelection))
    };

    // Bulk enable/disable: update each selected job, surface one summary toast.
    let bulk_set_enabled = {
        let state = state.clone();
        let toast = toast.clone();
        move |enabled: bool| {
            let state = state.clone();
            let toast = toast.clone();
            let ids: Vec<String> = state.selected.iter().cloned().collect();
            if ids.is_empty() {
                return;
            }
            spawn_local(async move {
                let mut ok = 0usize;
                let mut failed = 0usize;
                for id in ids {
                    let req = UpdateRequest {
                        enabled: Some(enabled),
                        ..Default::default()
                    };
                    match api_update(&id, &req).await {
                        Ok(job) => {
                            state.dispatch(CAction::Upsert(job));
                            ok += 1;
                        }
                        Err(_) => failed += 1,
                    }
                }
                let verb = if enabled { "enabled" } else { "disabled" };
                if failed == 0 {
                    toast.emit((format!("{ok} job(s) {verb}"), ToastKind::Ok));
                } else {
                    toast.emit((format!("{ok} {verb}, {failed} failed"), ToastKind::Err));
                }
            });
        }
    };

    let on_bulk_enable = {
        let f = bulk_set_enabled.clone();
        Callback::from(move |_: ()| f(true))
    };
    let on_bulk_disable = {
        let f = bulk_set_enabled.clone();
        Callback::from(move |_: ()| f(false))
    };

    let on_bulk_delete = {
        let state = state.clone();
        Callback::from(move |_: ()| state.dispatch(CAction::OpenConfirmBulkDelete))
    };

    let on_confirm_bulk_delete = {
        let state = state.clone();
        let toast = toast.clone();
        Callback::from(move |_: ()| {
            let state = state.clone();
            let toast = toast.clone();
            let ids: Vec<String> = state.selected.iter().cloned().collect();
            state.dispatch(CAction::CloseModal);
            spawn_local(async move {
                let mut deleted = Vec::new();
                let mut failed = 0usize;
                for id in ids {
                    match api_delete(&id).await {
                        Ok(()) => deleted.push(id),
                        Err(_) => failed += 1,
                    }
                }
                let n = deleted.len();
                state.dispatch(CAction::RemoveMany(deleted));
                if failed == 0 {
                    toast.emit((format!("{n} job(s) deleted"), ToastKind::Ok));
                } else {
                    toast.emit((format!("{n} deleted, {failed} failed"), ToastKind::Err));
                }
            });
        })
    };

    let on_set_view = {
        let state = state.clone();
        Callback::from(move |view: CView| state.dispatch(CAction::SetView(view)))
    };

    let jobs = state.jobs.clone();
    let loading = state.loading;
    let view = state.view;
    let page = state.page.clone();
    let modal = state.modal.clone();
    let selected = state.selected.clone();

    let edit_job = match &modal {
        CModal::Edit(id) => jobs.iter().find(|j| j.id == *id).cloned(),
        _ => None,
    };

    html! {
        <>
            {
                match page {
                    CPage::New => html! {
                        <CreatePage on_cancel={on_cancel} on_save={on_create} />
                    },
                    CPage::Logs(id) => {
                        let handler = jobs.iter()
                            .find(|j| j.id == id)
                            .map(|j| j.handler.clone())
                            .unwrap_or_default();
                        html! {
                            <LogsPage job_id={id} handler={handler} on_back={on_back} />
                        }
                    },
                    CPage::List => html! {
                        <main>
                            <StatsBar jobs={jobs.clone()} />
                            <div class="section-hd">
                                <div class="section-label">{"SCHEDULED JOBS"}</div>
                                <div class="section-acts">
                                    <CronViewToggle view={view} on_set_view={on_set_view} />
                                    <button class="btn btn-primary btn-sm" onclick={on_new}>{"+ NEW JOB"}</button>
                                </div>
                            </div>
                            {
                                match view {
                                    CView::Table => html! {
                                        <>
                                            <BulkBar
                                                count={selected.len()}
                                                on_enable={on_bulk_enable}
                                                on_disable={on_bulk_disable}
                                                on_delete={on_bulk_delete}
                                                on_clear={on_clear_selection}
                                            />
                                            <JobTable
                                                jobs={jobs}
                                                loading={loading}
                                                selected={selected}
                                                on_edit={on_edit}
                                                on_delete={on_ask_delete}
                                                on_toggle={on_toggle}
                                                on_trigger={on_trigger}
                                                on_logs={on_logs}
                                                on_select={on_select}
                                                on_select_all={on_select_all}
                                            />
                                        </>
                                    },
                                    CView::Day => {
                                        let items = jobs.iter().filter(|j| j.enabled).map(|j| TimelineItem {
                                            label: j.handler.clone(),
                                            schedule: j.schedule.clone(),
                                        }).collect::<Vec<_>>();
                                        html! { <DayTimeline items={items} loading={loading} /> }
                                    },
                                }
                            }
                        </main>
                    },
                }
            }
            {
                match &modal {
                    CModal::Edit(_) => html! {
                        <JobModal
                            editing={edit_job}
                            on_close={on_close.clone()}
                            on_save={on_save}
                        />
                    },
                    CModal::ConfirmDelete { id, handler } => html! {
                        <ConfirmDialog
                            job_id={id.clone()}
                            handler={handler.clone()}
                            on_cancel={on_close.clone()}
                            on_confirm={on_confirm_delete}
                        />
                    },
                    CModal::ConfirmBulkDelete { count } => html! {
                        <BulkDeleteDialog
                            count={*count}
                            on_cancel={on_close}
                            on_confirm={on_confirm_bulk_delete}
                        />
                    },
                    CModal::None => html! {},
                }
            }
        </>
    }
}

// ─── View toggle ──────────────────────────────────────────────────────────────

#[derive(Properties, PartialEq)]
pub struct CronViewToggleProps {
    pub view: CView,
    pub on_set_view: Callback<CView>,
}

/// Table / Day switch for the cron-jobs list page.
#[function_component(CronViewToggle)]
pub fn cron_view_toggle(props: &CronViewToggleProps) -> Html {
    let mk = |view: CView, label: &'static str| {
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
            { mk(CView::Table, "LIST") }
            { mk(CView::Day, "DAY") }
        </div>
    }
}

// ─── Stats bar ────────────────────────────────────────────────────────────────

#[derive(Properties, PartialEq)]
pub struct StatsProps {
    pub jobs: Vec<CronJob>,
}

#[function_component(StatsBar)]
pub fn stats_bar(props: &StatsProps) -> Html {
    let total = props.jobs.len();
    let enabled = props.jobs.iter().filter(|j| j.enabled).count();
    let disabled = total - enabled;

    html! {
        <div class="stats">
            <div class="stat-card all">
                <div class="stat-label">{"TOTAL JOBS"}</div>
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
        </div>
    }
}

// ─── Job table ────────────────────────────────────────────────────────────────

#[derive(Properties, PartialEq)]
pub struct JobTableProps {
    pub jobs: Vec<CronJob>,
    pub loading: bool,
    pub selected: HashSet<String>,
    pub on_edit: Callback<String>,
    pub on_delete: Callback<(String, String)>,
    pub on_toggle: Callback<(String, bool)>,
    pub on_trigger: Callback<String>,
    pub on_logs: Callback<String>,
    pub on_select: Callback<(String, SelectKind)>,
    pub on_select_all: Callback<()>,
}

#[function_component(JobTable)]
pub fn job_table(props: &JobTableProps) -> Html {
    if props.loading {
        return html! {
            <div class="table-wrap">
                <div class="empty"><div class="spinner"></div></div>
            </div>
        };
    }
    if props.jobs.is_empty() {
        return html! {
            <div class="table-wrap">
                <div class="empty">
                    <div class="empty-icon">{"⧗"}</div>
                    <div class="empty-msg">{"NO JOBS SCHEDULED"}</div>
                    <div class="empty-sub">{"press + NEW JOB to create one"}</div>
                </div>
            </div>
        };
    }

    let all_selected = !props.jobs.is_empty() && props.selected.len() == props.jobs.len();
    let on_select_all = {
        let cb = props.on_select_all.clone();
        Callback::from(move |_: MouseEvent| cb.emit(()))
    };

    html! {
        <div class="table-wrap">
            <table>
                <thead>
                    <tr>
                        <th class="th-select">
                            <input
                                type="checkbox"
                                class="row-check"
                                title="Select all"
                                aria-label="Select all jobs"
                                checked={all_selected}
                                onclick={on_select_all}
                            />
                        </th>
                        <th>{"ID"}</th>
                        <th>{"SCHEDULE"}</th>
                        <th>{"HANDLER"}</th>
                        <th>{"METADATA"}</th>
                        <th>{"ENABLED"}</th>
                        <th>{"UPDATED"}</th>
                        <th></th>
                    </tr>
                </thead>
                <tbody>
                    { for props.jobs.iter().map(|job| html! {
                        <JobRow
                            key={job.id.clone()}
                            job={job.clone()}
                            selected={props.selected.contains(&job.id)}
                            on_edit={props.on_edit.clone()}
                            on_delete={props.on_delete.clone()}
                            on_toggle={props.on_toggle.clone()}
                            on_trigger={props.on_trigger.clone()}
                            on_logs={props.on_logs.clone()}
                            on_select={props.on_select.clone()}
                        />
                    }) }
                </tbody>
            </table>
        </div>
    }
}

// ─── Job row ──────────────────────────────────────────────────────────────────

#[derive(Properties, PartialEq)]
pub struct JobRowProps {
    pub job: CronJob,
    pub selected: bool,
    pub on_edit: Callback<String>,
    pub on_delete: Callback<(String, String)>,
    pub on_toggle: Callback<(String, bool)>,
    pub on_trigger: Callback<String>,
    pub on_logs: Callback<String>,
    pub on_select: Callback<(String, SelectKind)>,
}

#[function_component(JobRow)]
pub fn job_row(props: &JobRowProps) -> Html {
    let job = &props.job;
    let id_short = format!("{}…", &job.id[..8.min(job.id.len())]);
    let cron_text = job
        .schedule_description
        .as_deref()
        .unwrap_or("—")
        .to_string();
    let meta = meta_preview(&job.metadata);
    let updated = reltime(job.updated_at);

    let on_edit = {
        let cb = props.on_edit.clone();
        let id = job.id.clone();
        Callback::from(move |_: MouseEvent| cb.emit(id.clone()))
    };
    let on_delete = {
        let cb = props.on_delete.clone();
        let id = job.id.clone();
        let handler = job.handler.clone();
        Callback::from(move |_: MouseEvent| cb.emit((id.clone(), handler.clone())))
    };
    let on_toggle = {
        let cb = props.on_toggle.clone();
        let id = job.id.clone();
        let enabled = job.enabled;
        Callback::from(move |_: Event| cb.emit((id.clone(), !enabled)))
    };
    let on_trigger = {
        let cb = props.on_trigger.clone();
        let id = job.id.clone();
        Callback::from(move |_: MouseEvent| cb.emit(id.clone()))
    };
    let on_logs = {
        let cb = props.on_logs.clone();
        let id = job.id.clone();
        Callback::from(move |_: MouseEvent| cb.emit(id.clone()))
    };
    let on_select = {
        let cb = props.on_select.clone();
        let id = job.id.clone();
        Callback::from(move |e: MouseEvent| {
            let kind = if e.shift_key() {
                SelectKind::Range
            } else if e.ctrl_key() || e.meta_key() {
                SelectKind::Toggle
            } else {
                SelectKind::Only
            };
            cb.emit((id.clone(), kind));
        })
    };

    let last_run = job
        .last_manual_trigger_at
        .map(|t| format!("↻ {}", reltime(t)))
        .unwrap_or_default();

    let row_class = if props.selected { "row-selected" } else { "" };

    html! {
        <tr class={row_class}>
            <td class="td-select">
                <input
                    type="checkbox"
                    class="row-check"
                    aria-label="Select job"
                    checked={props.selected}
                    onclick={on_select}
                />
            </td>
            <td><span class="cell-id" title={job.id.clone()}>{id_short}</span></td>
            <td>
                <div class="cell-schedule">{&job.schedule}</div>
                <div class="cell-schedule-human">{cron_text}</div>
            </td>
            <td><span class="cell-handler">{&job.handler}</span></td>
            <td><span class="cell-meta">{meta}</span></td>
            <td>
                <label class="toggle">
                    <input type="checkbox" checked={job.enabled} onchange={on_toggle} />
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
                    <button class="act-btn run" title="Run now" aria-label="Run now" onclick={on_trigger}>{"▶"}</button>
                    <button class="act-btn logs" onclick={on_logs}>{"LOGS"}</button>
                    <button class="act-btn edit" onclick={on_edit}>{"EDIT"}</button>
                    <button class="act-btn del" title="Delete job" aria-label="Delete job" onclick={on_delete}>{"✕"}</button>
                </div>
            </td>
        </tr>
    }
}

// ─── Create page ──────────────────────────────────────────────────────────────

#[derive(Properties, PartialEq)]
pub struct CreatePageProps {
    pub on_cancel: Callback<()>,
    pub on_save: Callback<CreateRequest>,
}

#[function_component(CreatePage)]
pub fn create_page(props: &CreatePageProps) -> Html {
    let schedule = use_state(String::new);
    let handler = use_state(String::new);
    let meta_raw = use_state(String::new);
    let machines = use_state(Vec::<String>::new);
    let enabled = use_state(|| true);
    let meta_err = use_state(String::new);
    let saving = use_state(|| false);

    let (cron_ok, cron_text) = describe_cron_live(&schedule);

    let on_schedule = {
        let schedule = schedule.clone();
        Callback::from(move |e: InputEvent| {
            let input: HtmlInputElement = e.target_unchecked_into();
            schedule.set(input.value());
        })
    };
    let on_handler = {
        let handler = handler.clone();
        Callback::from(move |e: InputEvent| {
            let input: HtmlInputElement = e.target_unchecked_into();
            handler.set(input.value());
        })
    };
    let on_meta = {
        let meta_raw = meta_raw.clone();
        let meta_err = meta_err.clone();
        Callback::from(move |e: InputEvent| {
            let input: HtmlInputElement = e.target_unchecked_into();
            let val = input.value();
            if val.trim().is_empty() {
                meta_err.set(String::new());
            } else if let Err(err) = serde_json::from_str::<Json>(&val) {
                meta_err.set(format!("↳ {err}"));
            } else {
                meta_err.set(String::new());
            }
            meta_raw.set(val);
        })
    };
    let on_machines = {
        let machines = machines.clone();
        Callback::from(move |next: Vec<String>| machines.set(next))
    };
    let on_enabled = {
        let enabled = enabled.clone();
        Callback::from(move |e: Event| {
            let input: HtmlInputElement = e.target_unchecked_into();
            enabled.set(input.checked());
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
    let on_save_click = {
        let schedule = schedule.clone();
        let handler = handler.clone();
        let meta_raw = meta_raw.clone();
        let machines = machines.clone();
        let meta_err = meta_err.clone();
        let enabled = enabled.clone();
        let saving = saving.clone();
        let cb = props.on_save.clone();
        Callback::from(move |_: MouseEvent| {
            if !meta_err.is_empty() {
                return;
            }
            let metadata = if meta_raw.trim().is_empty() {
                Json::Null
            } else {
                serde_json::from_str(&meta_raw).unwrap_or(Json::Null)
            };
            saving.set(true);
            cb.emit(CreateRequest {
                schedule: (*schedule).clone(),
                handler: (*handler).clone(),
                metadata,
                machines: (*machines).clone(),
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

    let meta_class = if !meta_err.is_empty() {
        "form-input invalid"
    } else {
        "form-input"
    };

    html! {
        <main class="create-page">
            <div class="page-hd">
                <button class="btn btn-ghost btn-sm" onclick={on_cancel_click.clone()}>{"← BACK"}</button>
                <div class="page-title">{"NEW JOB"}</div>
            </div>
            <div class="page-card">
                <div class="modal-body">
                    <div class="form-group">
                        <label class="form-label">
                            {"SCHEDULE "}
                            <span class="form-required">{"*"}</span>
                        </label>
                        <input
                            class="form-input"
                            type="text"
                            placeholder="sec min hour dom month dow year"
                            value={(*schedule).clone()}
                            oninput={on_schedule}
                            autocomplete="off"
                            spellcheck="false"
                        />
                        <div class="cron-presets">
                            { for [
                                ("@daily", "@daily"), ("@hourly", "@hourly"),
                                ("@weekly", "@weekly"), ("@monthly", "@monthly"),
                                ("0 0 9 * * 1-5 *", "weekdays 9am"),
                                ("0 */15 * * * * *", "every 15min"),
                                ("0 0 * * * * *", "every hour"),
                                ("0 0 0 1 * * *", "monthly"),
                            ].iter().map(|(val, label)| html! {
                                <button class="preset-btn" onclick={set_preset(val)}>{*label}</button>
                            }) }
                        </div>
                        <div class={preview_class}>{cron_text}</div>
                    </div>
                    <div class="form-group">
                        <label class="form-label">
                            {"HANDLER "}
                            <span class="form-required">{"*"}</span>
                        </label>
                        <input
                            class="form-input"
                            type="text"
                            placeholder="send-report"
                            value={(*handler).clone()}
                            oninput={on_handler}
                            autocomplete="off"
                            spellcheck="false"
                        />
                    </div>
                    <div class="form-group">
                        <label class="form-label">
                            {"METADATA "}
                            <span style="color:var(--text-ghost)">{"(JSON)"}</span>
                        </label>
                        <textarea
                            class={meta_class}
                            placeholder={r#"{"recipient": "team@example.com"}"#}
                            value={(*meta_raw).clone()}
                            oninput={on_meta}
                        />
                        if !meta_err.is_empty() {
                            <div class="field-err">{(*meta_err).clone()}</div>
                        }
                    </div>
                    <MachinesPicker value={(*machines).clone()} on_change={on_machines} />
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
                <div class="modal-ft">
                    <button class="btn btn-ghost btn-sm" onclick={on_cancel_click}>{"CANCEL"}</button>
                    <button
                        class="btn btn-primary btn-sm"
                        onclick={on_save_click}
                        disabled={*saving}
                    >
                        { if *saving { "…" } else { "CREATE JOB" } }
                    </button>
                </div>
            </div>
        </main>
    }
}

// ─── Job modal (edit only) ────────────────────────────────────────────────────

#[derive(Properties, PartialEq)]
pub struct JobModalProps {
    pub editing: Option<CronJob>,
    pub on_close: Callback<()>,
    pub on_save: Callback<CreateRequest>,
}

#[function_component(JobModal)]
pub fn job_modal(props: &JobModalProps) -> Html {
    let schedule = use_state(|| {
        props
            .editing
            .as_ref()
            .map(|j| j.schedule.clone())
            .unwrap_or_default()
    });
    let handler = use_state(|| {
        props
            .editing
            .as_ref()
            .map(|j| j.handler.clone())
            .unwrap_or_default()
    });
    let meta_raw = use_state(|| {
        props
            .editing
            .as_ref()
            .and_then(|j| {
                if j.metadata.is_null() {
                    None
                } else {
                    serde_json::to_string_pretty(&j.metadata).ok()
                }
            })
            .unwrap_or_default()
    });
    let machines = use_state(|| {
        props
            .editing
            .as_ref()
            .map(|j| j.machines.clone())
            .unwrap_or_default()
    });
    let enabled = use_state(|| props.editing.as_ref().map(|j| j.enabled).unwrap_or(true));
    let meta_err = use_state(String::new);
    let saving = use_state(|| false);

    let (cron_ok, cron_text) = describe_cron_live(&schedule);

    let on_schedule = {
        let schedule = schedule.clone();
        Callback::from(move |e: InputEvent| {
            let input: HtmlInputElement = e.target_unchecked_into();
            schedule.set(input.value());
        })
    };
    let on_handler = {
        let handler = handler.clone();
        Callback::from(move |e: InputEvent| {
            let input: HtmlInputElement = e.target_unchecked_into();
            handler.set(input.value());
        })
    };
    let on_meta = {
        let meta_raw = meta_raw.clone();
        let meta_err = meta_err.clone();
        Callback::from(move |e: InputEvent| {
            let input: HtmlInputElement = e.target_unchecked_into();
            let val = input.value();
            if val.trim().is_empty() {
                meta_err.set(String::new());
            } else if let Err(err) = serde_json::from_str::<Json>(&val) {
                meta_err.set(format!("↳ {err}"));
            } else {
                meta_err.set(String::new());
            }
            meta_raw.set(val);
        })
    };
    let on_enabled = {
        let enabled = enabled.clone();
        Callback::from(move |e: Event| {
            let input: HtmlInputElement = e.target_unchecked_into();
            enabled.set(input.checked());
        })
    };

    let set_preset = |val: &'static str| {
        let schedule = schedule.clone();
        Callback::from(move |_: MouseEvent| schedule.set(val.to_string()))
    };

    let on_machines = {
        let machines = machines.clone();
        Callback::from(move |next: Vec<String>| machines.set(next))
    };

    let on_close_click = {
        let cb = props.on_close.clone();
        Callback::from(move |_: MouseEvent| cb.emit(()))
    };
    let on_save_click = {
        let schedule = schedule.clone();
        let handler = handler.clone();
        let meta_raw = meta_raw.clone();
        let machines = machines.clone();
        let meta_err = meta_err.clone();
        let enabled = enabled.clone();
        let saving = saving.clone();
        let cb = props.on_save.clone();
        Callback::from(move |_: MouseEvent| {
            if !meta_err.is_empty() {
                return;
            }
            let metadata = if meta_raw.trim().is_empty() {
                Json::Null
            } else {
                serde_json::from_str(&meta_raw).unwrap_or(Json::Null)
            };
            saving.set(true);
            cb.emit(CreateRequest {
                schedule: (*schedule).clone(),
                handler: (*handler).clone(),
                metadata,
                machines: (*machines).clone(),
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

    let meta_class = if !meta_err.is_empty() {
        "form-input invalid"
    } else {
        "form-input"
    };

    html! {
        <div class="overlay open">
            <div class="modal">
                <div class="modal-hd">
                    <div class="modal-title">{"EDIT JOB"}</div>
                    <button class="modal-x" title="Close" aria-label="Close" onclick={on_close_click.clone()}>{"✕"}</button>
                </div>
                <div class="modal-body">
                    <div class="form-group">
                        <label class="form-label">
                            {"SCHEDULE "}
                            <span class="form-required">{"*"}</span>
                        </label>
                        <input
                            class="form-input"
                            type="text"
                            placeholder="sec min hour dom month dow year"
                            value={(*schedule).clone()}
                            oninput={on_schedule}
                            autocomplete="off"
                            spellcheck="false"
                        />
                        <div class="cron-presets">
                            { for [
                                ("@daily", "@daily"), ("@hourly", "@hourly"),
                                ("@weekly", "@weekly"), ("@monthly", "@monthly"),
                                ("0 0 9 * * 1-5 *", "weekdays 9am"),
                                ("0 */15 * * * * *", "every 15min"),
                                ("0 0 * * * * *", "every hour"),
                                ("0 0 0 1 * * *", "monthly"),
                            ].iter().map(|(val, label)| html! {
                                <button class="preset-btn" onclick={set_preset(val)}>{*label}</button>
                            }) }
                        </div>
                        <div class={preview_class}>{cron_text}</div>
                    </div>
                    <div class="form-group">
                        <label class="form-label">
                            {"HANDLER "}
                            <span class="form-required">{"*"}</span>
                        </label>
                        <input
                            class="form-input"
                            type="text"
                            placeholder="send-report"
                            value={(*handler).clone()}
                            oninput={on_handler}
                            autocomplete="off"
                            spellcheck="false"
                        />
                    </div>
                    <div class="form-group">
                        <label class="form-label">
                            {"METADATA "}
                            <span style="color:var(--text-ghost)">{"(JSON)"}</span>
                        </label>
                        <textarea
                            class={meta_class}
                            placeholder={r#"{"recipient": "team@example.com"}"#}
                            value={(*meta_raw).clone()}
                            oninput={on_meta}
                        />
                        if !meta_err.is_empty() {
                            <div class="field-err">{(*meta_err).clone()}</div>
                        }
                    </div>
                    <MachinesPicker value={(*machines).clone()} on_change={on_machines} />
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
                <div class="modal-ft">
                    <button class="btn btn-ghost btn-sm" onclick={on_close_click}>{"CANCEL"}</button>
                    <button
                        class="btn btn-primary btn-sm"
                        onclick={on_save_click}
                        disabled={*saving}
                    >
                        { if *saving { "…" } else { "SAVE CHANGES" } }
                    </button>
                </div>
            </div>
        </div>
    }
}

// ─── Confirm dialog ───────────────────────────────────────────────────────────

#[derive(Properties, PartialEq)]
pub struct ConfirmProps {
    pub job_id: String,
    pub handler: String,
    pub on_cancel: Callback<()>,
    pub on_confirm: Callback<String>,
}

#[function_component(ConfirmDialog)]
pub fn confirm_dialog(props: &ConfirmProps) -> Html {
    let on_cancel = {
        let cb = props.on_cancel.clone();
        Callback::from(move |_: MouseEvent| cb.emit(()))
    };
    let on_confirm = {
        let cb = props.on_confirm.clone();
        let id = props.job_id.clone();
        Callback::from(move |_: MouseEvent| cb.emit(id.clone()))
    };

    html! {
        <div class="overlay open">
            <div class="confirm-dialog">
                <div class="confirm-title">{"⚠ DELETE JOB"}</div>
                <div class="confirm-msg">
                    { format!("Delete the job running \"{}\"? This cannot be undone.", props.handler) }
                </div>
                <div class="confirm-acts">
                    <button class="btn btn-ghost btn-sm" onclick={on_cancel}>{"CANCEL"}</button>
                    <button class="btn btn-danger btn-sm" onclick={on_confirm}>{"DELETE"}</button>
                </div>
            </div>
        </div>
    }
}

// ─── Bulk action bar ──────────────────────────────────────────────────────────

#[derive(Properties, PartialEq)]
pub struct BulkBarProps {
    pub count: usize,
    pub on_enable: Callback<()>,
    pub on_disable: Callback<()>,
    pub on_delete: Callback<()>,
    pub on_clear: Callback<()>,
}

#[function_component(BulkBar)]
pub fn bulk_bar(props: &BulkBarProps) -> Html {
    // Hidden entirely until at least one row is selected.
    if props.count == 0 {
        return html! {};
    }
    let mk = |cb: Callback<()>| Callback::from(move |_: MouseEvent| cb.emit(()));

    html! {
        <div class="bulk-bar">
            <span class="bulk-count">{ format!("{} SELECTED", props.count) }</span>
            <div class="bulk-acts">
                <button class="btn btn-ghost btn-sm" onclick={mk(props.on_enable.clone())}>{"ENABLE"}</button>
                <button class="btn btn-ghost btn-sm" onclick={mk(props.on_disable.clone())}>{"DISABLE"}</button>
                <button class="btn btn-danger btn-sm" onclick={mk(props.on_delete.clone())}>{"DELETE"}</button>
                <button class="btn btn-ghost btn-sm" onclick={mk(props.on_clear.clone())}>{"CLEAR"}</button>
            </div>
        </div>
    }
}

// ─── Bulk delete confirm dialog ───────────────────────────────────────────────

#[derive(Properties, PartialEq)]
pub struct BulkDeleteProps {
    pub count: usize,
    pub on_cancel: Callback<()>,
    pub on_confirm: Callback<()>,
}

#[function_component(BulkDeleteDialog)]
pub fn bulk_delete_dialog(props: &BulkDeleteProps) -> Html {
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
            <div class="confirm-dialog">
                <div class="confirm-title">{"⚠ DELETE JOBS"}</div>
                <div class="confirm-msg">
                    { format!("Delete {} selected job(s)? This cannot be undone.", props.count) }
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
pub struct LogsPageProps {
    pub job_id: String,
    pub handler: String,
    pub on_back: Callback<()>,
}

#[function_component(LogsPage)]
pub fn logs_page(props: &LogsPageProps) -> Html {
    let content: UseStateHandle<Option<String>> = use_state(|| None);
    let loading = use_state(|| true);
    let err: UseStateHandle<Option<String>> = use_state(|| None);

    {
        let id = props.job_id.clone();
        let content = content.clone();
        let loading = loading.clone();
        let err = err.clone();
        use_effect_with(id.clone(), move |id| {
            let id = id.clone();
            let content = content.clone();
            let loading = loading.clone();
            let err = err.clone();
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
        });
    }

    let on_back = {
        let cb = props.on_back.clone();
        Callback::from(move |_: MouseEvent| cb.emit(()))
    };

    let on_refresh = {
        let id = props.job_id.clone();
        let content = content.clone();
        let loading = loading.clone();
        let err = err.clone();
        Callback::from(move |_: MouseEvent| {
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
        })
    };

    let is_loading = *loading;
    let err_msg = (*err).clone();
    let log_text = (*content).clone();

    let body = if is_loading {
        html! { <div class="empty"><div class="spinner"></div></div> }
    } else if let Some(e) = err_msg {
        html! { <div class="logs-error">{format!("Error: {e}")}</div> }
    } else if let Some(text) = log_text {
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
                <div class="page-title">{format!("LOGS / {}", props.handler)}</div>
                <button class="btn-refresh" title="Refresh" aria-label="Refresh" onclick={on_refresh}>{"↻"}</button>
            </div>
            <div class="logs-wrap">
                {body}
            </div>
        </main>
    }
}

// ─── Utilities ────────────────────────────────────────────────────────────────

fn meta_preview(v: &Json) -> String {
    match v {
        Json::Null => "—".into(),
        Json::Object(o) if o.is_empty() => "{}".into(),
        Json::Object(o) => {
            let k = o.len();
            if k == 1 {
                format!("{{{}}}", o.keys().next().unwrap())
            } else {
                format!("{{{k} keys}}")
            }
        }
        other => other.to_string().chars().take(24).collect(),
    }
}
