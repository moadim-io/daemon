//! Routines tab: list, create, edit, trigger, logs, and delete agent-driven scheduled jobs.
//!
//! Mirrors the cron-jobs UI but targets the `/routines` API. A routine launches an AI agent
//! (claude, codex, …) on a schedule instead of running a handler script.

use std::cell::Cell;
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

use chrono::{DateTime, Datelike, Duration, Local, NaiveDate, TimeZone};
use gloo_net::http::Request;
use gloo_timers::future::TimeoutFuture;
use serde::{Deserialize, Serialize};
use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::spawn_local;
use web_sys::{HtmlElement, HtmlInputElement, HtmlSelectElement, KeyboardEvent};
use yew::prelude::*;

use crate::day_timeline::{DayTimeline, TimelineItem};
use crate::log_viewer::LogViewer;
use crate::machines::MachinesPicker;
use crate::refresh::{RefreshControl, RefreshInterval};
use crate::schedule::{fires_within, fmt_until, fmt_when, next_fire_after};
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
    /// Machines this routine runs on. An empty list runs nowhere (dormant until assigned).
    #[serde(default)]
    pub machines: Vec<String>,
    pub enabled: bool,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub created_at: u64,
    #[serde(default)]
    pub updated_at: u64,
    #[serde(default)]
    pub last_manual_trigger_at: Option<u64>,
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
    /// Machines to run this routine on (empty = runs nowhere until assigned).
    pub machines: Vec<String>,
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
    pub machines: Option<Vec<String>>,
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

// ─── Faceted filter ───────────────────────────────────────────────────────────
//
// Pure, host-testable filtering of the loaded routines. The view binds a search
// box, status facet, agent facet, and machine facet to a `RoutineFilter`; the
// table and day timeline render `filter_routines(...)` instead of the raw list.
// Best-practice (Airflow/Buildkite/GitHub Actions dashboards): free-text + facets
// narrow a dense list, a live result count keeps the active filter legible, and
// clicking a KPI tile cross-filters the detail table.

/// How far ahead a routine's next fire counts as "due soon" for the KPI tile.
pub(crate) const DUE_SOON_WINDOW_SECS: i64 = 3_600;
/// Tick cadence for the live "now" handle (keeps DueSoon count fresh between fetches).
const NEXT_RUN_TICK_MS: u32 = 30_000;

/// Enabled / disabled / dormant / due-soon status facet for routines.
/// `Dormant` means enabled but with an empty machines list — it will never fire.
/// `DueSoon` means enabled with a next fire within [`DUE_SOON_WINDOW_SECS`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RoutineStatusFacet {
    #[default]
    All,
    Enabled,
    Disabled,
    Dormant,
    DueSoon,
}

impl RoutineStatusFacet {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            RoutineStatusFacet::All => "all",
            RoutineStatusFacet::Enabled => "enabled",
            RoutineStatusFacet::Disabled => "disabled",
            RoutineStatusFacet::Dormant => "dormant",
            RoutineStatusFacet::DueSoon => "due",
        }
    }

    #[must_use]
    pub fn from_str(s: &str) -> Self {
        match s {
            "enabled" => RoutineStatusFacet::Enabled,
            "disabled" => RoutineStatusFacet::Disabled,
            "dormant" => RoutineStatusFacet::Dormant,
            "due" => RoutineStatusFacet::DueSoon,
            _ => RoutineStatusFacet::All,
        }
    }
}

/// Sentinel select values for the machine facet. Real machine ids never collide
/// with these (no leading NUL in user-supplied names).
const RMACHINE_ANY: &str = "\u{0}any";
const RMACHINE_UNASSIGNED: &str = "\u{0}unassigned";

/// Machine facet for the routines filter.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum RoutineMachineFacet {
    #[default]
    Any,
    Unassigned,
    Machine(String),
}

impl RoutineMachineFacet {
    #[must_use]
    pub fn as_value(&self) -> String {
        match self {
            RoutineMachineFacet::Any => RMACHINE_ANY.to_string(),
            RoutineMachineFacet::Unassigned => RMACHINE_UNASSIGNED.to_string(),
            RoutineMachineFacet::Machine(m) => m.clone(),
        }
    }

    #[must_use]
    pub fn from_value(v: &str) -> Self {
        match v {
            RMACHINE_ANY => RoutineMachineFacet::Any,
            RMACHINE_UNASSIGNED => RoutineMachineFacet::Unassigned,
            other => RoutineMachineFacet::Machine(other.to_string()),
        }
    }
}

/// Agent facet: all agents, or one specific agent name.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum AgentFacet {
    #[default]
    All,
    Named(String),
}

impl AgentFacet {
    const AGENT_ALL: &'static str = "\u{0}all";

    #[must_use]
    pub fn as_value(&self) -> String {
        match self {
            AgentFacet::All => Self::AGENT_ALL.to_string(),
            AgentFacet::Named(a) => a.clone(),
        }
    }

    #[must_use]
    pub fn from_value(v: &str) -> Self {
        if v == Self::AGENT_ALL {
            AgentFacet::All
        } else {
            AgentFacet::Named(v.to_string())
        }
    }
}

/// Combined free-text + faceted filter applied client-side to the loaded routines.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct RoutineFilter {
    /// Free-text needle matched across title, agent, prompt, repositories,
    /// schedule, and schedule_description.
    pub query: String,
    pub status: RoutineStatusFacet,
    pub agent: AgentFacet,
    pub machine: RoutineMachineFacet,
}

impl RoutineFilter {
    /// `true` when at least one facet is narrowing the list.
    #[must_use]
    pub fn is_active(&self) -> bool {
        !self.query.trim().is_empty()
            || self.status != RoutineStatusFacet::All
            || self.agent != AgentFacet::All
            || self.machine != RoutineMachineFacet::Any
    }

    /// Does this routine survive the filter? Facets AND together.
    /// `now` and `window` are used only when the `DueSoon` status facet is active.
    #[must_use]
    pub fn matches(&self, r: &Routine, now: DateTime<Local>, window: Duration) -> bool {
        match self.status {
            RoutineStatusFacet::All => {}
            RoutineStatusFacet::Enabled if !r.enabled => return false,
            RoutineStatusFacet::Disabled if r.enabled => return false,
            RoutineStatusFacet::Dormant if !(r.enabled && r.machines.is_empty()) => return false,
            RoutineStatusFacet::DueSoon
                if !(r.enabled && fires_within(&r.schedule, now, window)) =>
            {
                return false
            }
            _ => {}
        }
        match &self.agent {
            AgentFacet::All => {}
            AgentFacet::Named(a) if r.agent != *a => return false,
            _ => {}
        }
        match &self.machine {
            RoutineMachineFacet::Any => {}
            RoutineMachineFacet::Unassigned if !r.machines.is_empty() => return false,
            RoutineMachineFacet::Machine(m) if !r.machines.iter().any(|x| x == m) => return false,
            _ => {}
        }
        let q = self.query.trim().to_lowercase();
        if !q.is_empty() {
            let repos = r
                .repositories
                .iter()
                .map(|repo| repo.repository.to_lowercase())
                .collect::<Vec<_>>()
                .join(" ");
            let desc = r
                .schedule_description
                .as_deref()
                .unwrap_or_default()
                .to_lowercase();
            let hay = format!(
                "{} {} {} {} {}",
                r.title.to_lowercase(),
                r.agent.to_lowercase(),
                r.schedule.to_lowercase(),
                repos,
                desc,
            );
            if !hay.contains(&q) {
                return false;
            }
        }
        true
    }
}

/// Routines surviving `filter`, preserving the input order.
#[must_use]
pub fn filter_routines(
    routines: &[Routine],
    filter: &RoutineFilter,
    now: DateTime<Local>,
    window: Duration,
) -> Vec<Routine> {
    routines
        .iter()
        .filter(|r| filter.matches(r, now, window))
        .cloned()
        .collect()
}

/// Distinct agent names across all routines, sorted.
#[must_use]
pub fn distinct_agents(routines: &[Routine]) -> Vec<String> {
    let mut set: BTreeSet<String> = BTreeSet::new();
    for r in routines {
        set.insert(r.agent.clone());
    }
    set.into_iter().collect()
}

/// Distinct machine ids across all routines, sorted.
#[must_use]
pub fn distinct_machines_r(routines: &[Routine]) -> Vec<String> {
    let mut set: BTreeSet<String> = BTreeSet::new();
    for r in routines {
        for m in &r.machines {
            set.insert(m.clone());
        }
    }
    set.into_iter().collect()
}

/// Count of routines with no machine assigned.
#[must_use]
pub fn unassigned_routines_count(routines: &[Routine]) -> usize {
    routines.iter().filter(|r| r.machines.is_empty()).count()
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
    Day,
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

/// Dimension to group the routines table by.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GroupBy {
    /// No grouping — flat sorted list (default).
    #[default]
    None,
    /// Group by the routine's first machine (`"UNASSIGNED"` when `machines` is empty).
    Machine,
    /// Group by `routine.agent`.
    Agent,
    /// Two groups: `ENABLED` first, `DISABLED` second; empty groups are omitted.
    Enabled,
}

impl GroupBy {
    pub(crate) fn from_str(s: &str) -> Self {
        match s {
            "machine" => GroupBy::Machine,
            "agent" => GroupBy::Agent,
            "enabled" => GroupBy::Enabled,
            _ => GroupBy::None,
        }
    }

    pub(crate) fn as_str(self) -> &'static str {
        match self {
            GroupBy::None => "none",
            GroupBy::Machine => "machine",
            GroupBy::Agent => "agent",
            GroupBy::Enabled => "enabled",
        }
    }
}

/// Partition `items` (already filtered and sorted) into labelled groups.
///
/// - [`GroupBy::None`]: a single group with label `""` — caller skips group headers.
/// - [`GroupBy::Machine`]: groups by the first entry in `routine.machines`
///   (`"UNASSIGNED"` when the list is empty); groups sorted A→Z.
/// - [`GroupBy::Agent`]: groups by `routine.agent`, sorted A→Z.
/// - [`GroupBy::Enabled`]: `"ENABLED"` first, then `"DISABLED"`; empty groups omitted.
pub(crate) fn group_routines(items: Vec<Routine>, by: GroupBy) -> Vec<(String, Vec<Routine>)> {
    match by {
        GroupBy::None => vec![("".to_string(), items)],
        GroupBy::Machine => {
            let mut map: BTreeMap<String, Vec<Routine>> = BTreeMap::new();
            for r in items {
                let key = r
                    .machines
                    .first()
                    .cloned()
                    .unwrap_or_else(|| "UNASSIGNED".to_string());
                map.entry(key).or_default().push(r);
            }
            map.into_iter().collect()
        }
        GroupBy::Agent => {
            let mut map: BTreeMap<String, Vec<Routine>> = BTreeMap::new();
            for r in items {
                map.entry(r.agent.clone()).or_default().push(r);
            }
            map.into_iter().collect()
        }
        GroupBy::Enabled => {
            let (enabled, disabled): (Vec<_>, Vec<_>) =
                items.into_iter().partition(|r| r.enabled);
            let mut groups: Vec<(String, Vec<Routine>)> = Vec::new();
            if !enabled.is_empty() {
                groups.push(("ENABLED".to_string(), enabled));
            }
            if !disabled.is_empty() {
                groups.push(("DISABLED".to_string(), disabled));
            }
            groups
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
    /// Active faceted filter.
    pub filter: RoutineFilter,
    /// Field the table is sorted by.
    pub sort: RSort,
    /// `true` sorts descending (newest / Z→A first).
    pub sort_desc: bool,
    /// Active grouping dimension for the table view.
    pub group_by: GroupBy,
}

impl Default for RState {
    fn default() -> Self {
        Self {
            routines: vec![],
            loading: true,
            page: RPage::List,
            modal: RModal::None,
            view: RView::default(),
            filter: RoutineFilter::default(),
            sort: RSort::default(),
            sort_desc: false,
            group_by: GroupBy::default(),
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
    SetQuery(String),
    SetStatusFacet(RoutineStatusFacet),
    SetAgentFacet(AgentFacet),
    SetMachineFacet(RoutineMachineFacet),
    ClearFilters,
    SetSort(RSort),
    ToggleSortDir,
    SetGroupBy(GroupBy),
    Upsert(Box<Routine>),
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
            RAction::SetQuery(q) => s.filter.query = q,
            RAction::SetStatusFacet(st) => s.filter.status = st,
            RAction::SetAgentFacet(ag) => s.filter.agent = ag,
            RAction::SetMachineFacet(m) => s.filter.machine = m,
            RAction::ClearFilters => s.filter = RoutineFilter::default(),
            RAction::SetSort(sort) => s.sort = sort,
            RAction::ToggleSortDir => s.sort_desc = !s.sort_desc,
            RAction::SetGroupBy(gb) => s.group_by = gb,
            RAction::Upsert(routine) => {
                let routine = *routine;
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

    // Live "now" advanced on a fixed tick so DUE SOON counts stay current.
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
                    Ok(r) => {
                        state.dispatch(RAction::Loaded(r));
                        updated_at.set(js_sys::Date::now());
                    }
                    Err(e) => toast.emit((format!("Failed to load routines: {e}"), ToastKind::Err)),
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
                            Ok(r) => {
                                if cancelled.get() {
                                    break;
                                }
                                state.dispatch(RAction::Loaded(r));
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
    let on_set_query = {
        let state = state.clone();
        Callback::from(move |q: String| state.dispatch(RAction::SetQuery(q)))
    };
    let on_set_status = {
        let state = state.clone();
        Callback::from(move |st: RoutineStatusFacet| state.dispatch(RAction::SetStatusFacet(st)))
    };
    let on_set_agent = {
        let state = state.clone();
        Callback::from(move |ag: AgentFacet| state.dispatch(RAction::SetAgentFacet(ag)))
    };
    let on_set_machine = {
        let state = state.clone();
        Callback::from(move |m: RoutineMachineFacet| state.dispatch(RAction::SetMachineFacet(m)))
    };
    let on_clear_filters = {
        let state = state.clone();
        Callback::from(move |_: ()| state.dispatch(RAction::ClearFilters))
    };

    // `/` focuses the search box (while not already typing in another field),
    // matching the GitHub/Slack convention and complementing the ⌘K palette.
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

    let on_set_sort = {
        let state = state.clone();
        Callback::from(move |sort: RSort| state.dispatch(RAction::SetSort(sort)))
    };
    let on_toggle_sort_dir = {
        let state = state.clone();
        Callback::from(move |_: ()| state.dispatch(RAction::ToggleSortDir))
    };
    let on_set_group_by = {
        let state = state.clone();
        Callback::from(move |gb: GroupBy| state.dispatch(RAction::SetGroupBy(gb)))
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
                        state.dispatch(RAction::Upsert(Box::new(r)));
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
                        state.dispatch(RAction::Upsert(Box::new(r)));
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
                        state.dispatch(RAction::Upsert(Box::new(r)));
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
                        machines: Some(req.machines),
                        enabled: Some(req.enabled),
                        ttl_secs: req.ttl_secs,
                    };
                    match api_update(id, &upd).await {
                        Ok(r) => {
                            state.dispatch(RAction::Upsert(Box::new(r)));
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
    let filter = state.filter.clone();
    let sort = state.sort;
    let sort_desc = state.sort_desc;
    let group_by = state.group_by;

    // Faceted filter + sort applied client-side.
    let now_val = *now;
    let window = Duration::seconds(DUE_SOON_WINDOW_SECS);
    let total_routines = routines.len();
    let agent_options = distinct_agents(&routines);
    let machine_options = distinct_machines_r(&routines);
    let has_unassigned = unassigned_routines_count(&routines) > 0;
    let filter_active = filter.is_active();
    let visible = {
        let mut v = filter_routines(&routines, &filter, now_val, window);
        match sort {
            RSort::Created => v.sort_by_key(|r| r.created_at),
            RSort::Updated => v.sort_by_key(|r| r.updated_at),
            RSort::Title => v.sort_by_key(|r| r.title.to_lowercase()),
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
    let shown = visible.len();

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
                            <RoutineStatsBar
                                routines={routines.clone()}
                                now={now_val}
                                active={filter.status}
                                on_status={on_set_status.clone()}
                            />
                            <div class="section-hd">
                                <div class="section-label">{"SCHEDULED ROUTINES"}</div>
                                <div class="section-acts">
                                    <RefreshControl
                                        interval={*interval}
                                        updated_at_ms={*updated_at}
                                        on_change={on_set_interval}
                                    />
                                    <ViewToggle view={view} on_set_view={on_set_view} />
                                    <button class="btn btn-ghost btn-sm" onclick={on_cleanup}
                                        title="Reap finished, expired run workbenches now">{"CLEANUP NOW"}</button>
                                    <button class="btn btn-primary btn-sm" onclick={on_new}>{"+ NEW ROUTINE"}</button>
                                </div>
                            </div>
                            <FilterSortBar
                                filter={filter.clone()}
                                agents={agent_options}
                                machines={machine_options}
                                has_unassigned={has_unassigned}
                                shown={shown}
                                total={total_routines}
                                sort={sort}
                                sort_desc={sort_desc}
                                group_by={group_by}
                                search_ref={search_ref.clone()}
                                on_query={on_set_query}
                                on_status={on_set_status}
                                on_agent={on_set_agent}
                                on_machine={on_set_machine}
                                on_clear={on_clear_filters.clone()}
                                on_set_sort={on_set_sort}
                                on_toggle_sort_dir={on_toggle_sort_dir}
                                on_set_group_by={on_set_group_by}
                            />
                            {
                                match view {
                                    RView::Table => html! {
                                        <RoutineTable
                                            routines={visible}
                                            loading={loading}
                                            filter_active={filter_active}
                                            group_by={group_by}
                                            now={now_val}
                                            on_edit={on_edit}
                                            on_delete={on_ask_delete}
                                            on_toggle={on_toggle}
                                            on_trigger={on_trigger}
                                            on_logs={on_logs}
                                            on_clear_filters={on_clear_filters}
                                        />
                                    },
                                    RView::Calendar => html! {
                                        <RoutineCalendar routines={visible} loading={loading} />
                                    },
                                    RView::Day => {
                                        let items = visible.iter().filter(|r| r.enabled).map(|r| TimelineItem {
                                            label: r.title.clone(),
                                            schedule: r.schedule.clone(),
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
pub struct StatsBarProps {
    pub routines: Vec<Routine>,
    /// "Now" used to compute the DueSoon count.
    pub now: DateTime<Local>,
    /// Currently active status facet — drives `aria-pressed`.
    pub active: RoutineStatusFacet,
    /// Fired when the user clicks a tile; pass `All` to clear the facet.
    pub on_status: Callback<RoutineStatusFacet>,
}

/// Cross-filterable KPI stat tiles for the Routines page.
///
/// Clicking ENABLED / DISABLED / DUE SOON applies (or clears, if already active)
/// the matching status facet on the list below, matching the Cron Jobs page pattern.
#[function_component(RoutineStatsBar)]
pub fn routine_stats_bar(props: &StatsBarProps) -> Html {
    let window = Duration::seconds(DUE_SOON_WINDOW_SECS);
    let total = props.routines.len();
    let enabled = props.routines.iter().filter(|r| r.enabled).count();
    let disabled = total - enabled;
    let due_soon = props
        .routines
        .iter()
        .filter(|r| r.enabled && fires_within(&r.schedule, props.now, window))
        .count();
    let unreg = props
        .routines
        .iter()
        .filter(|r| !r.agent_registered)
        .count();

    let mk =
        |facet: RoutineStatusFacet, label: &'static str, val: usize, extra_cls: &'static str| {
            let cb = props.on_status.clone();
            let active = props.active;
            let pressed = active == facet;
            // Toggle: clicking the active tile clears the filter (resets to All).
            let target = if pressed {
                RoutineStatusFacet::All
            } else {
                facet
            };
            let mut cls = format!("stat-card {extra_cls}");
            if pressed {
                cls.push_str(" active");
            }
            html! {
                <button type="button" class={cls}
                    aria-pressed={pressed.to_string()}
                    onclick={Callback::from(move |_: MouseEvent| cb.emit(target))}>
                    <div class="stat-label">{label}</div>
                    <div class="stat-val">{val}</div>
                </button>
            }
        };

    html! {
        <div class="stats">
            { mk(RoutineStatusFacet::All, "TOTAL", total, "all") }
            { mk(RoutineStatusFacet::Enabled, "ENABLED", enabled, "enabled") }
            { mk(RoutineStatusFacet::Disabled, "DISABLED", disabled, "disabled") }
            { mk(RoutineStatusFacet::DueSoon, "DUE SOON", due_soon, "due") }
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
            { mk(RView::Day, "DAY") }
        </div>
    }
}

// ─── Filter & sort bar ────────────────────────────────────────────────────────

#[derive(Properties, PartialEq)]
pub struct FilterSortBarProps {
    pub filter: RoutineFilter,
    /// Distinct agent names across all routines, for the agent-facet options.
    pub agents: Vec<String>,
    /// Distinct machine ids across all routines, for the machine-facet options.
    pub machines: Vec<String>,
    /// Whether at least one dormant (no-machine) routine exists.
    pub has_unassigned: bool,
    /// Count after filtering / total loaded — rendered as "Showing N of M".
    pub shown: usize,
    pub total: usize,
    pub sort: RSort,
    pub sort_desc: bool,
    /// Active grouping dimension for the table.
    pub group_by: GroupBy,
    /// NodeRef forwarded from the page so the `/` shortcut can focus this input.
    pub search_ref: NodeRef,
    pub on_query: Callback<String>,
    pub on_status: Callback<RoutineStatusFacet>,
    pub on_agent: Callback<AgentFacet>,
    pub on_machine: Callback<RoutineMachineFacet>,
    pub on_clear: Callback<()>,
    pub on_set_sort: Callback<RSort>,
    pub on_toggle_sort_dir: Callback<()>,
    pub on_set_group_by: Callback<GroupBy>,
}

/// Full-text search + status / agent / machine facets + sort controls for the routine table.
#[function_component(FilterSortBar)]
pub fn filter_sort_bar(props: &FilterSortBarProps) -> Html {
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
            cb.emit(RoutineStatusFacet::from_str(&select.value()));
        })
    };
    let on_agent_change = {
        let cb = props.on_agent.clone();
        Callback::from(move |e: Event| {
            let select: HtmlSelectElement = e.target_unchecked_into();
            cb.emit(AgentFacet::from_value(&select.value()));
        })
    };
    let on_machine_change = {
        let cb = props.on_machine.clone();
        Callback::from(move |e: Event| {
            let select: HtmlSelectElement = e.target_unchecked_into();
            cb.emit(RoutineMachineFacet::from_value(&select.value()));
        })
    };
    let on_clear = {
        let cb = props.on_clear.clone();
        Callback::from(move |_: MouseEvent| cb.emit(()))
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
    let on_group_by_change = {
        let cb = props.on_set_group_by.clone();
        Callback::from(move |e: Event| {
            let select: HtmlSelectElement = e.target_unchecked_into();
            cb.emit(GroupBy::from_str(&select.value()));
        })
    };

    let dir_label = if props.sort_desc {
        "↓ DESC"
    } else {
        "↑ ASC"
    };
    let current_sort = props.sort.as_str();
    let current_group = props.group_by.as_str();
    let status_val = props.filter.status.as_str();
    let agent_val = props.filter.agent.as_value();
    let machine_val = props.filter.machine.as_value();
    let active = props.filter.is_active();

    html! {
        <div class="filter-bar">
            <div class="filter-field">
                <input
                    ref={props.search_ref.clone()}
                    type="text"
                    class="filter-input"
                    placeholder="Search routines…  ( / )"
                    aria-label="Search routines"
                    value={props.filter.query.clone()}
                    oninput={on_input}
                />
                <span class="filter-label">{"STATUS"}</span>
                <select class="filter-select" aria-label="Status filter" onchange={on_status_change}>
                    <option value="all" selected={status_val == "all"}>{"All"}</option>
                    <option value="enabled" selected={status_val == "enabled"}>{"Enabled"}</option>
                    <option value="disabled" selected={status_val == "disabled"}>{"Disabled"}</option>
                    <option value="dormant" selected={status_val == "dormant"}>{"Dormant"}</option>
                    <option value="due" selected={status_val == "due"}>{"Due soon"}</option>
                </select>
                <span class="filter-label">{"AGENT"}</span>
                <select class="filter-select" aria-label="Agent filter" onchange={on_agent_change}>
                    <option value={AgentFacet::AGENT_ALL} selected={agent_val == AgentFacet::AGENT_ALL}>{"Any"}</option>
                    { for props.agents.iter().map(|a| html! {
                        <option value={a.clone()} selected={agent_val == *a}>{a.clone()}</option>
                    }) }
                </select>
                <span class="filter-label">{"MACHINE"}</span>
                <select class="filter-select" aria-label="Machine filter" onchange={on_machine_change}>
                    <option value={RMACHINE_ANY} selected={machine_val == RMACHINE_ANY}>{"Any"}</option>
                    {
                        if props.has_unassigned {
                            html! {
                                <option value={RMACHINE_UNASSIGNED}
                                    selected={machine_val == RMACHINE_UNASSIGNED}>{"Unassigned"}</option>
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
                <span class="filter-label">{"SORT"}</span>
                <select class="filter-select" onchange={on_sort_change}>
                    <option value="created" selected={current_sort == "created"}>{"Created"}</option>
                    <option value="updated" selected={current_sort == "updated"}>{"Updated"}</option>
                    <option value="title" selected={current_sort == "title"}>{"Title"}</option>
                    <option value="repository" selected={current_sort == "repository"}>{"Repository"}</option>
                </select>
                <button class="btn btn-ghost btn-sm" onclick={on_dir}
                    title="Toggle sort direction">{dir_label}</button>
                <span class="filter-label">{"GROUP"}</span>
                <select class="filter-select" aria-label="Group by" onchange={on_group_by_change}>
                    <option value="none" selected={current_group == "none"}>{"None"}</option>
                    <option value="machine" selected={current_group == "machine"}>{"Machine"}</option>
                    <option value="agent" selected={current_group == "agent"}>{"Agent"}</option>
                    <option value="enabled" selected={current_group == "enabled"}>{"Status"}</option>
                </select>
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
                <button class="btn-refresh" title="Previous month" aria-label="Previous month" onclick={on_prev}>{"‹"}</button>
                <div class="cal-month">{month_label}</div>
                <button class="btn-refresh" title="Next month" aria-label="Next month" onclick={on_next}>{"›"}</button>
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
    /// Whether a filter is narrowing the list — selects the filtered-empty state.
    pub filter_active: bool,
    /// Active grouping dimension; `GroupBy::None` renders a flat tbody.
    pub group_by: GroupBy,
    /// Reference instant used to compute next-fire countdowns.
    pub now: chrono::DateTime<Local>,
    pub on_edit: Callback<String>,
    pub on_delete: Callback<(String, String)>,
    pub on_toggle: Callback<(String, bool)>,
    pub on_trigger: Callback<String>,
    pub on_logs: Callback<String>,
    pub on_clear_filters: Callback<()>,
}

#[function_component(RoutineTable)]
pub fn routine_table(props: &TableProps) -> Html {
    if props.loading {
        return html! {
            <div class="table-wrap"><div class="empty"><div class="spinner"></div></div></div>
        };
    }
    if props.routines.is_empty() {
        let (icon, msg, sub) = if props.filter_active {
            let on_clear = {
                let cb = props.on_clear_filters.clone();
                Callback::from(move |_: MouseEvent| cb.emit(()))
            };
            return html! {
                <div class="table-wrap">
                    <div class="empty">
                        <div class="empty-icon">{"⊘"}</div>
                        <div class="empty-msg">{"NO ROUTINES MATCH"}</div>
                        <div class="empty-sub">
                            <button class="btn btn-ghost btn-sm" onclick={on_clear}>{"CLEAR FILTERS"}</button>
                        </div>
                    </div>
                </div>
            };
        } else {
            (
                "⧗",
                "NO ROUTINES SCHEDULED",
                "press + NEW ROUTINE to create one",
            )
        };
        return html! {
            <div class="table-wrap">
                <div class="empty">
                    <div class="empty-icon">{icon}</div>
                    <div class="empty-msg">{msg}</div>
                    <div class="empty-sub">{sub}</div>
                </div>
            </div>
        };
    }

    let make_row = |r: &Routine| {
        html! {
            <RoutineRow
                key={r.id.clone()}
                routine={r.clone()}
                now={props.now}
                on_edit={props.on_edit.clone()}
                on_delete={props.on_delete.clone()}
                on_toggle={props.on_toggle.clone()}
                on_trigger={props.on_trigger.clone()}
                on_logs={props.on_logs.clone()}
            />
        }
    };

    let body = if props.group_by == GroupBy::None {
        html! {
            <tbody>
                { for props.routines.iter().map(&make_row) }
            </tbody>
        }
    } else {
        let groups = group_routines(props.routines.clone(), props.group_by);
        html! {
            <>
                { for groups.into_iter().map(|(label, items)| {
                    let count = items.len();
                    html! {
                        <tbody>
                            <tr class="group-row">
                                <td colspan="9">
                                    {format!("{} ({})", label, count)}
                                </td>
                            </tr>
                            { for items.iter().map(&make_row) }
                        </tbody>
                    }
                }) }
            </>
        }
    };

    html! {
        <div class="table-wrap">
            <table>
                <thead>
                    <tr>
                        <th>{"TITLE"}</th>
                        <th>{"SCHEDULE"}</th>
                        <th>{"NEXT RUN"}</th>
                        <th>{"AGENT"}</th>
                        <th>{"REPOS"}</th>
                        <th>{"TTL"}</th>
                        <th>{"ENABLED"}</th>
                        <th>{"UPDATED"}</th>
                        <th></th>
                    </tr>
                </thead>
                {body}
            </table>
        </div>
    }
}

/// Render a routine's NEXT RUN cell: "paused" when disabled, an absolute time
/// plus a relative countdown when its schedule has a future fire, or "—" for
/// an invalid/exhausted schedule. The countdown gets a `soon` accent inside the
/// due-soon window, matching the Overview KPI tile.
pub(crate) fn next_routine_run_cell(routine: &Routine, now: chrono::DateTime<Local>) -> Html {
    if !routine.enabled {
        return html! { <span class="cell-next muted">{"paused"}</span> };
    }
    match next_fire_after(&routine.schedule, now) {
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

#[derive(Properties, PartialEq)]
pub struct RowProps {
    pub routine: Routine,
    /// Reference instant for the NEXT RUN countdown.
    pub now: chrono::DateTime<Local>,
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

    let last_run: Html = {
        let manual = r.last_manual_trigger_at;
        let scheduled = r.last_scheduled_trigger_at;
        match (manual, scheduled) {
            (None, None) => html! {
                <div class="cell-triggered" style="color:var(--text-ghost)">{"never fired"}</div>
            },
            (Some(m), Some(s)) if m >= s => html! {
                <div class="cell-triggered">{format!("↻ {}", reltime(m))}</div>
            },
            (Some(_m), Some(s)) => html! {
                <div class="cell-triggered">{format!("⏱ {}", reltime(s))}</div>
            },
            (Some(m), None) => html! {
                <div class="cell-triggered">{format!("↻ {}", reltime(m))}</div>
            },
            (None, Some(s)) => html! {
                <div class="cell-triggered">{format!("⏱ {}", reltime(s))}</div>
            },
        }
    };

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

    let next_run = next_routine_run_cell(r, props.now);

    html! {
        <tr>
            <td>
                <div class="cell-schedule" title={r.id.clone()}>{&r.title}</div>
            </td>
            <td>
                <div class="cell-schedule">{&r.schedule}</div>
                <div class="cell-schedule-human">{cron_text}</div>
            </td>
            <td>{next_run}</td>
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
                {last_run}
            </td>
            <td>
                <div class="row-actions">
                    <button class="act-btn run" title="Run now" aria-label="Run now" onclick={on_trigger}>{"▶"}</button>
                    <button class="act-btn logs" onclick={on_logs}>{"LOGS"}</button>
                    <button class="act-btn edit" onclick={on_edit}>{"EDIT"}</button>
                    <button class="act-btn del" title="Delete routine" aria-label="Delete routine" onclick={on_delete}>{"✕"}</button>
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
    let machines = use_state(|| {
        editing
            .as_ref()
            .map(|r| r.machines.clone())
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
    let on_machines = {
        let machines = machines.clone();
        Callback::from(move |next: Vec<String>| machines.set(next))
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
        let machines = machines.clone();
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
                machines: (*machines).clone(),
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
            <MachinesPicker value={(*machines).clone()} on_change={on_machines} />
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
                        <button class="modal-x" title="Close" aria-label="Close" onclick={on_cancel_click}>{"✕"}</button>
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

    html! {
        <main class="logs-page">
            <div class="page-hd">
                <button class="btn btn-ghost btn-sm" onclick={on_back}>{"← BACK"}</button>
                <div class="page-title">{format!("LOGS / {}", props.title)}</div>
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

#[cfg(test)]
#[path = "routines_tests.rs"]
mod routines_tests;
