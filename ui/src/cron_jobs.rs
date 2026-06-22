//! Cron-jobs page: list, create, edit, trigger, logs, and delete schedule-driven handler jobs.
//!
//! Self-contained like [`crate::routines::RoutinesPage`]: owns its own reducer state and talks to
//! the `/cron-jobs` API. Toasts bubble up to the shell via the `on_toast` callback.

use std::cell::Cell;
use std::collections::{BTreeSet, HashSet};
use std::rc::Rc;

use chrono::{DateTime, Duration, Local};
use gloo_net::http::Request;
use gloo_timers::future::TimeoutFuture;
use serde::{Deserialize, Serialize};
use serde_json::Value as Json;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::spawn_local;
use web_sys::{HtmlElement, HtmlInputElement, HtmlSelectElement, KeyboardEvent};
use yew::prelude::*;

use crate::day_timeline::{DayTimeline, TimelineItem};
use crate::machines::MachinesPicker;
use crate::refresh::{RefreshControl, RefreshInterval};
use crate::schedule::{fires_within, fmt_until, fmt_when, next_fire_after};
use crate::{describe_cron_live, reltime, ToastKind};

/// How long ahead a job's next fire counts as "due soon" for the KPI tile.
const DUE_SOON_WINDOW_SECS: i64 = 3_600;
/// Cadence of the live tick that keeps next-run countdowns and the due-soon
/// count current without a manual reload.
const NEXT_RUN_TICK_MS: u32 = 30_000;

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

// ─── Faceted filter ─────────────────────────────────────────────────────────
//
// Pure, host-testable filtering of the loaded jobs. The view binds a search box,
// a status facet, and a machine facet to a `JobFilter`; the table and day
// timeline render `filter_jobs(...)` instead of the raw list. Best-practice
// (Datadog/Grafana/BI dashboards): free-text + facets narrow a dense list, a
// live result count keeps the active filter legible, and clicking a summary KPI
// cross-filters the detail list.

/// Enabled/disabled/due-soon status facet. `DueSoon` reuses the same
/// `fires_within` window that backs the DUE SOON KPI tile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StatusFacet {
    #[default]
    All,
    Enabled,
    Disabled,
    DueSoon,
}

impl StatusFacet {
    /// Stable token used as the cross-filter id and the segmented-control value.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            StatusFacet::All => "all",
            StatusFacet::Enabled => "enabled",
            StatusFacet::Disabled => "disabled",
            StatusFacet::DueSoon => "due",
        }
    }

    #[must_use]
    pub fn from_str(s: &str) -> Self {
        match s {
            "enabled" => StatusFacet::Enabled,
            "disabled" => StatusFacet::Disabled,
            "due" => StatusFacet::DueSoon,
            _ => StatusFacet::All,
        }
    }
}

/// Sentinel select values for the machine facet's non-machine choices. Real
/// machine ids never collide with these (they carry no leading NUL).
const MACHINE_ANY: &str = "\u{0}any";
const MACHINE_UNASSIGNED: &str = "\u{0}unassigned";

/// Machine facet: any machine, the dormant (no-machine) jobs, or one specific
/// machine drawn from the distinct machines across the loaded jobs.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum MachineFacet {
    #[default]
    Any,
    Unassigned,
    Machine(String),
}

impl MachineFacet {
    /// Encode for the `<select>` option value.
    #[must_use]
    pub fn as_value(&self) -> String {
        match self {
            MachineFacet::Any => MACHINE_ANY.to_string(),
            MachineFacet::Unassigned => MACHINE_UNASSIGNED.to_string(),
            MachineFacet::Machine(m) => m.clone(),
        }
    }

    /// Decode from a selected `<select>` option value.
    #[must_use]
    pub fn from_value(v: &str) -> Self {
        match v {
            MACHINE_ANY => MachineFacet::Any,
            MACHINE_UNASSIGNED => MachineFacet::Unassigned,
            other => MachineFacet::Machine(other.to_string()),
        }
    }
}

/// Combined free-text + facet filter applied client-side to the loaded jobs.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct JobFilter {
    pub query: String,
    pub status: StatusFacet,
    pub machine: MachineFacet,
}

impl JobFilter {
    /// Whether any facet is narrowing the list — drives the "Clear filters"
    /// affordance and the filter-aware empty state.
    #[must_use]
    pub fn is_active(&self) -> bool {
        !self.query.trim().is_empty()
            || self.status != StatusFacet::All
            || self.machine != MachineFacet::Any
    }

    /// Does this job survive the filter? Facets AND together; free-text matches
    /// across id, handler, schedule, the human description, and metadata.
    #[must_use]
    pub fn matches(&self, job: &CronJob, now: DateTime<Local>, window: Duration) -> bool {
        match self.status {
            StatusFacet::All => {}
            StatusFacet::Enabled if !job.enabled => return false,
            StatusFacet::Disabled if job.enabled => return false,
            StatusFacet::DueSoon if !(job.enabled && fires_within(&job.schedule, now, window)) => {
                return false
            }
            _ => {}
        }
        match &self.machine {
            MachineFacet::Any => {}
            MachineFacet::Unassigned if !job.machines.is_empty() => return false,
            MachineFacet::Machine(m) if !job.machines.iter().any(|x| x == m) => return false,
            _ => {}
        }
        let q = self.query.trim().to_lowercase();
        if !q.is_empty() {
            let desc = job
                .schedule_description
                .as_deref()
                .unwrap_or_default()
                .to_lowercase();
            let hay = format!(
                "{} {} {} {} {}",
                job.id.to_lowercase(),
                job.handler.to_lowercase(),
                job.schedule.to_lowercase(),
                desc,
                job.metadata.to_string().to_lowercase(),
            );
            if !hay.contains(&q) {
                return false;
            }
        }
        true
    }
}

/// Jobs surviving `filter`, preserving the input order.
#[must_use]
pub fn filter_jobs(
    jobs: &[CronJob],
    filter: &JobFilter,
    now: DateTime<Local>,
    window: Duration,
) -> Vec<CronJob> {
    jobs.iter()
        .filter(|j| filter.matches(j, now, window))
        .cloned()
        .collect()
}

/// Distinct machine ids across all jobs, sorted, for the machine-facet options.
#[must_use]
pub fn distinct_machines(jobs: &[CronJob]) -> Vec<String> {
    let mut set: BTreeSet<String> = BTreeSet::new();
    for j in jobs {
        for m in &j.machines {
            set.insert(m.clone());
        }
    }
    set.into_iter().collect()
}

/// Count of dormant jobs (no machine assigned) — surfaced as the "Unassigned"
/// machine-facet option only when at least one such job exists.
#[must_use]
pub fn unassigned_count(jobs: &[CronJob]) -> usize {
    jobs.iter().filter(|j| j.machines.is_empty()).count()
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
    /// Active faceted filter applied to the list/day views.
    pub filter: JobFilter,
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
            filter: JobFilter::default(),
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
    /// Select exactly the given (visible/filtered) ids.
    SelectAll(Vec<String>),
    ClearSelection,
    SetQuery(String),
    SetStatus(StatusFacet),
    SetMachineFacet(MachineFacet),
    ClearFilters,
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
            CAction::SelectAll(ids) => {
                s.selected = ids.into_iter().collect();
            }
            CAction::ClearSelection => {
                s.selected.clear();
                s.select_anchor = None;
            }
            CAction::SetQuery(q) => s.filter.query = q,
            CAction::SetStatus(status) => s.filter.status = status,
            CAction::SetMachineFacet(m) => s.filter.machine = m,
            CAction::ClearFilters => s.filter = JobFilter::default(),
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

    // Live "now", advanced on a fixed tick so next-run countdowns and the
    // due-soon KPI stay current without a manual reload. Both the stats bar and
    // the table read this same instant so the view is internally consistent.
    let now = use_state(Local::now);
    {
        let now = now.clone();
        use_effect_with((), move |_| {
            spawn_local(async move {
                loop {
                    TimeoutFuture::new(NEXT_RUN_TICK_MS).await;
                    now.set(Local::now());
                }
            });
        });
    }

    // Operator-chosen auto-refresh cadence (persisted), and the `Date.now()`
    // (ms) of the last successful list load that drives the freshness cue.
    let interval = use_state(crate::refresh::load_interval);
    let updated_at = use_state(|| 0.0_f64);

    let ok_toast = {
        let toast = toast.clone();
        move |msg: &str| toast.emit((msg.to_string(), ToastKind::Ok))
    };

    // Load on mount.
    {
        let state = state.clone();
        let toast = toast.clone();
        let updated_at = updated_at.clone();
        use_effect_with((), move |_| {
            spawn_local(async move {
                match api_list().await {
                    Ok(jobs) => {
                        state.dispatch(CAction::Loaded(jobs));
                        updated_at.set(js_sys::Date::now());
                    }
                    Err(e) => toast.emit((format!("Failed to load jobs: {e}"), ToastKind::Err)),
                }
            });
        });
    }

    // Auto-refresh loop, re-armed whenever the chosen interval changes. `Off`
    // installs no loop (today's load-once behaviour); any cadence re-fetches the
    // list on that period via the existing endpoint. The cleanup flag stops the
    // running loop when the interval changes or the page unmounts.
    {
        let state = state.clone();
        let toast = toast.clone();
        let updated_at = updated_at.clone();
        use_effect_with(*interval, move |interval| {
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
                            Ok(jobs) => {
                                if cancelled.get() {
                                    break;
                                }
                                state.dispatch(CAction::Loaded(jobs));
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

    let on_set_interval = {
        let interval = interval.clone();
        Callback::from(move |next: RefreshInterval| {
            crate::refresh::save_interval(next);
            interval.set(next);
        })
    };

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

    // Header checkbox toggles between "all visible selected" and "none". Operates
    // over the filtered rows, so select-all never reaches hidden jobs.
    let on_select_all = {
        let state = state.clone();
        let now = now.clone();
        Callback::from(move |_: ()| {
            let window = Duration::seconds(DUE_SOON_WINDOW_SECS);
            let visible = filter_jobs(&state.jobs, &state.filter, *now, window);
            let all_visible_selected =
                !visible.is_empty() && visible.iter().all(|j| state.selected.contains(&j.id));
            if all_visible_selected {
                state.dispatch(CAction::ClearSelection);
            } else {
                state.dispatch(CAction::SelectAll(
                    visible.into_iter().map(|j| j.id).collect(),
                ));
            }
        })
    };

    let on_clear_selection = {
        let state = state.clone();
        Callback::from(move |_: ()| state.dispatch(CAction::ClearSelection))
    };

    // ── Faceted filter ──
    let on_set_query = {
        let state = state.clone();
        Callback::from(move |q: String| state.dispatch(CAction::SetQuery(q)))
    };
    let on_set_status = {
        let state = state.clone();
        Callback::from(move |status: StatusFacet| state.dispatch(CAction::SetStatus(status)))
    };
    let on_set_machine = {
        let state = state.clone();
        Callback::from(move |m: MachineFacet| state.dispatch(CAction::SetMachineFacet(m)))
    };
    let on_clear_filters = {
        let state = state.clone();
        Callback::from(move |_: ()| state.dispatch(CAction::ClearFilters))
    };

    // `/` focuses the search box from anywhere on the page (skipped while the
    // user is already typing in a field), matching the GitHub/Slack convention
    // and complementing the ⌘K command palette.
    let search_ref = use_node_ref();
    {
        let search_ref = search_ref.clone();
        use_effect_with((), move |_| {
            let on_key =
                Closure::<dyn Fn(KeyboardEvent)>::wrap(Box::new(move |event: KeyboardEvent| {
                    if event.key() != "/" || event.meta_key() || event.ctrl_key() || event.alt_key()
                    {
                        return;
                    }
                    // Don't steal "/" while the user is typing in another control.
                    let typing = event
                        .target()
                        .and_then(|t| t.dyn_into::<HtmlElement>().ok())
                        .map(|el| {
                            let tag = el.tag_name();
                            tag == "INPUT" || tag == "TEXTAREA" || tag == "SELECT"
                        })
                        .unwrap_or(false);
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
    let now_val = *now;
    let view = state.view;
    let page = state.page.clone();
    let modal = state.modal.clone();
    let selected = state.selected.clone();
    let filter = state.filter.clone();

    // Faceted view of the list: the table and day timeline render the filtered
    // set; the KPI tiles stay over the full fleet.
    let window = Duration::seconds(DUE_SOON_WINDOW_SECS);
    let filtered = filter_jobs(&jobs, &filter, now_val, window);
    let total_jobs = jobs.len();
    let shown = filtered.len();
    let machine_options = distinct_machines(&jobs);
    let has_unassigned = unassigned_count(&jobs) > 0;
    let filter_active = filter.is_active();

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
                            <StatsBar
                                jobs={jobs.clone()}
                                now={now_val}
                                active={filter.status}
                                on_status={on_set_status.clone()}
                            />
                            <div class="section-hd">
                                <div class="section-label">{"SCHEDULED JOBS"}</div>
                                <div class="section-acts">
                                    <RefreshControl
                                        interval={*interval}
                                        updated_at_ms={*updated_at}
                                        on_change={on_set_interval}
                                    />
                                    <CronViewToggle view={view} on_set_view={on_set_view} />
                                    <button class="btn btn-primary btn-sm" onclick={on_new}>{"+ NEW JOB"}</button>
                                </div>
                            </div>
                            <CronFilterBar
                                filter={filter.clone()}
                                machines={machine_options}
                                has_unassigned={has_unassigned}
                                shown={shown}
                                total={total_jobs}
                                search_ref={search_ref.clone()}
                                on_query={on_set_query}
                                on_status={on_set_status}
                                on_machine={on_set_machine}
                                on_clear={on_clear_filters.clone()}
                            />
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
                                                jobs={filtered}
                                                loading={loading}
                                                now={now_val}
                                                selected={selected}
                                                filter_active={filter_active}
                                                on_edit={on_edit}
                                                on_delete={on_ask_delete}
                                                on_toggle={on_toggle}
                                                on_trigger={on_trigger}
                                                on_logs={on_logs}
                                                on_select={on_select}
                                                on_select_all={on_select_all}
                                                on_clear_filters={on_clear_filters}
                                            />
                                        </>
                                    },
                                    CView::Day => {
                                        let items = filtered.iter().filter(|j| j.enabled).map(|j| TimelineItem {
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
    /// Reference instant for the "due soon" computation; advanced on a live tick.
    pub now: DateTime<Local>,
    /// Currently active status facet, so the matching tile reads as selected.
    pub active: StatusFacet,
    /// Cross-filter: clicking a tile applies its status facet (or clears it when
    /// the tile is already active).
    pub on_status: Callback<StatusFacet>,
}

#[function_component(StatsBar)]
pub fn stats_bar(props: &StatsProps) -> Html {
    let total = props.jobs.len();
    let enabled = props.jobs.iter().filter(|j| j.enabled).count();
    let disabled = total - enabled;
    // Enabled jobs whose next fire is within the due-soon window — the most
    // operationally urgent fact, surfaced as a first-class KPI tile.
    let window = Duration::seconds(DUE_SOON_WINDOW_SECS);
    let due_soon = props
        .jobs
        .iter()
        .filter(|j| j.enabled && fires_within(&j.schedule, props.now, window))
        .count();

    // Render one tile. Clicking toggles the status facet; the active tile (and
    // TOTAL while no facet is active) reads as pressed for a clear cross-filter.
    let active = props.active;
    let tile = |facet: StatusFacet,
                extra: &'static str,
                label: &'static str,
                val: usize,
                val_cls: &'static str| {
        let on_status = props.on_status.clone();
        let onclick = Callback::from(move |_: MouseEvent| {
            // Re-clicking the active facet clears it back to All.
            let next = if active == facet {
                StatusFacet::All
            } else {
                facet
            };
            on_status.emit(next);
        });
        let is_active = active == facet;
        let cls = if is_active {
            format!("stat-card {extra} active")
        } else {
            format!("stat-card {extra}")
        };
        html! {
            <button type="button" class={cls} onclick={onclick}
                aria-pressed={is_active.to_string()}
                title={format!("Filter: {label}")}>
                <div class="stat-label">{label}</div>
                <div class={classes!("stat-val", val_cls)}>{val}</div>
            </button>
        }
    };

    html! {
        <div class="stats">
            { tile(StatusFacet::All, "all", "TOTAL JOBS", total, "") }
            { tile(StatusFacet::Enabled, "enabled", "ENABLED", enabled, "c-accent") }
            { tile(StatusFacet::DueSoon, "due", "DUE SOON", due_soon, "c-red") }
            { tile(StatusFacet::Disabled, "disabled", "DISABLED", disabled, "c-amber") }
        </div>
    }
}

// ─── Filter bar ───────────────────────────────────────────────────────────────

#[derive(Properties, PartialEq)]
pub struct CronFilterBarProps {
    pub filter: JobFilter,
    /// Distinct machines across all jobs, for the machine-facet options.
    pub machines: Vec<String>,
    /// Whether at least one dormant (no-machine) job exists.
    pub has_unassigned: bool,
    /// Count after filtering / total loaded — rendered as "Showing N of M".
    pub shown: usize,
    pub total: usize,
    pub search_ref: NodeRef,
    pub on_query: Callback<String>,
    pub on_status: Callback<StatusFacet>,
    pub on_machine: Callback<MachineFacet>,
    pub on_clear: Callback<()>,
}

#[function_component(CronFilterBar)]
pub fn cron_filter_bar(props: &CronFilterBarProps) -> Html {
    let on_input = {
        let cb = props.on_query.clone();
        Callback::from(move |e: InputEvent| {
            let input: HtmlInputElement = e.target_unchecked_into();
            cb.emit(input.value());
        })
    };
    let on_status_change = {
        let cb = props.on_status.clone();
        Callback::from(move |e: Event| {
            let select: HtmlSelectElement = e.target_unchecked_into();
            cb.emit(StatusFacet::from_str(&select.value()));
        })
    };
    let on_machine_change = {
        let cb = props.on_machine.clone();
        Callback::from(move |e: Event| {
            let select: HtmlSelectElement = e.target_unchecked_into();
            cb.emit(MachineFacet::from_value(&select.value()));
        })
    };
    let on_clear = {
        let cb = props.on_clear.clone();
        Callback::from(move |_: MouseEvent| cb.emit(()))
    };

    let status = props.filter.status.as_str();
    let machine_val = props.filter.machine.as_value();
    let active = props.filter.is_active();

    html! {
        <div class="filter-bar">
            <div class="filter-field">
                <input
                    ref={props.search_ref.clone()}
                    type="text"
                    class="filter-input"
                    placeholder="Search jobs…  ( / )"
                    aria-label="Search jobs"
                    value={props.filter.query.clone()}
                    oninput={on_input}
                />
                <span class="filter-label">{"STATUS"}</span>
                <select class="filter-select" aria-label="Status filter" onchange={on_status_change}>
                    <option value="all" selected={status == "all"}>{"All"}</option>
                    <option value="enabled" selected={status == "enabled"}>{"Enabled"}</option>
                    <option value="disabled" selected={status == "disabled"}>{"Disabled"}</option>
                    <option value="due" selected={status == "due"}>{"Due soon"}</option>
                </select>
                <span class="filter-label">{"MACHINE"}</span>
                <select class="filter-select" aria-label="Machine filter" onchange={on_machine_change}>
                    <option value={MACHINE_ANY} selected={machine_val == MACHINE_ANY}>{"Any"}</option>
                    {
                        if props.has_unassigned {
                            html! {
                                <option value={MACHINE_UNASSIGNED}
                                    selected={machine_val == MACHINE_UNASSIGNED}>{"Unassigned"}</option>
                            }
                        } else {
                            html! {}
                        }
                    }
                    { for props.machines.iter().map(|m| html! {
                        <option value={m.clone()} selected={machine_val == *m}>{m.clone()}</option>
                    }) }
                </select>
            </div>
            <div class="filter-field">
                <span class="filter-count">
                    {format!("Showing {} of {}", props.shown, props.total)}
                </span>
                {
                    if active {
                        html! {
                            <button class="btn btn-ghost btn-sm" onclick={on_clear}
                                title="Clear all filters">{"CLEAR"}</button>
                        }
                    } else {
                        html! {}
                    }
                }
            </div>
        </div>
    }
}

// ─── Job table ────────────────────────────────────────────────────────────────

#[derive(Properties, PartialEq)]
pub struct JobTableProps {
    pub jobs: Vec<CronJob>,
    pub loading: bool,
    /// Reference instant for next-run cells; advanced on a live tick.
    pub now: DateTime<Local>,
    pub selected: HashSet<String>,
    /// Whether a filter is narrowing the list — selects the filtered-empty state.
    pub filter_active: bool,
    pub on_edit: Callback<String>,
    pub on_delete: Callback<(String, String)>,
    pub on_toggle: Callback<(String, bool)>,
    pub on_trigger: Callback<String>,
    pub on_logs: Callback<String>,
    pub on_select: Callback<(String, SelectKind)>,
    pub on_select_all: Callback<()>,
    pub on_clear_filters: Callback<()>,
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
        // Filter-aware empty state: "no matches" (with a clear action) is a
        // different message from the genuine "nothing scheduled" zero state, so
        // an operator is never left wondering "is it broken or just filtered?".
        if props.filter_active {
            let on_clear = {
                let cb = props.on_clear_filters.clone();
                Callback::from(move |_: MouseEvent| cb.emit(()))
            };
            return html! {
                <div class="table-wrap">
                    <div class="empty">
                        <div class="empty-icon">{"⦰"}</div>
                        <div class="empty-msg">{"NO MATCHING JOBS"}</div>
                        <div class="empty-sub">{"no jobs match the active filter"}</div>
                        <button class="btn btn-ghost btn-sm" onclick={on_clear}>{"CLEAR FILTERS"}</button>
                    </div>
                </div>
            };
        }
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
                        <th>{"NEXT RUN"}</th>
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
                            now={props.now}
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
    /// Reference instant for this row's next-run cell; advanced on a live tick.
    pub now: DateTime<Local>,
    pub selected: bool,
    pub on_edit: Callback<String>,
    pub on_delete: Callback<(String, String)>,
    pub on_toggle: Callback<(String, bool)>,
    pub on_trigger: Callback<String>,
    pub on_logs: Callback<String>,
    pub on_select: Callback<(String, SelectKind)>,
}

/// Render a job's NEXT RUN cell: `paused` when disabled, an absolute *when* plus
/// a relative countdown when its schedule fires again, or `—` when the schedule
/// is invalid or never fires. The countdown gets a `soon` accent once the fire
/// falls inside the due-soon window, matching the DUE SOON KPI tile.
fn next_run_cell(job: &CronJob, now: DateTime<Local>) -> Html {
    if !job.enabled {
        return html! { <span class="cell-next muted">{"paused"}</span> };
    }
    match next_fire_after(&job.schedule, now) {
        Some(then) => {
            let soon = then - now <= Duration::seconds(DUE_SOON_WINDOW_SECS);
            let until_cls = if soon {
                "cell-next-until soon"
            } else {
                "cell-next-until"
            };
            html! {
                <div class="cell-next">
                    <div class="cell-next-when">{fmt_when(now, then)}</div>
                    <div class={until_cls}>{fmt_until(now, then)}</div>
                </div>
            }
        }
        None => html! { <span class="cell-next muted">{"—"}</span> },
    }
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
    let next_run = next_run_cell(job, props.now);

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
            <td>{next_run}</td>
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

#[cfg(test)]
#[path = "cron_jobs_tests.rs"]
mod cron_jobs_tests;
