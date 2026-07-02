//! Routines tab: list, create, edit, trigger, logs, and delete agent-driven scheduled jobs.
//!
//! Targets the `/routines` API. A routine launches an AI agent (claude, codex, …) on a
//! schedule.

use std::cell::Cell;
use std::collections::BTreeSet;
use std::rc::Rc;

use chrono::{DateTime, Datelike, Duration, Local};
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
use crate::machines::{api_current_machine, MachinesPicker};
use crate::refresh::{RefreshControl, RefreshInterval};
use crate::schedule::{
    fires_within, fmt_until, fmt_when, month_start, next_fire_after, next_fires,
    occurrences_per_day, CAL_MONTHS, GRID_CELLS, WEEKDAYS,
};
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
    #[serde(default)]
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
    /// Unix timestamp until which scheduled fires are skipped, or `None`. Mutually exclusive with
    /// `skip_runs`.
    #[serde(default)]
    pub snoozed_until: Option<u64>,
    /// Count of upcoming scheduled fires still to skip, or `None`. Mutually exclusive with
    /// `snoozed_until`.
    #[serde(default)]
    pub skip_runs: Option<u32>,
    /// Workbench retention (seconds) for finished runs; `None` falls back to the server default.
    #[serde(default)]
    pub ttl_secs: Option<u64>,
    /// Free-form labels for the routine.
    #[serde(default)]
    pub tags: Vec<String>,
    // Derived (absent on the bare Routine returned by /trigger — default to safe values).
    #[serde(default)]
    pub agent_registered: bool,
    #[serde(default)]
    pub file_path: String,
    #[serde(default)]
    pub schedule_description: Option<String>,
    /// Number of open flags raised against this routine (see [`Flag`]).
    #[serde(default)]
    pub flag_count: usize,
}

/// Whether a flag file is committed to version control or kept machine-local.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FlagScope {
    General,
    Local,
}

/// A flag raised against a routine (mirrors the server `Flag`).
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Flag {
    pub filename: String,
    #[serde(rename = "type")]
    pub flag_type: String,
    pub description: String,
    pub scope: FlagScope,
    #[serde(default)]
    pub created_at: u64,
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
    /// Free-form labels for the routine.
    pub tags: Vec<String>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
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

/// Lock state as returned by `GET /api/v1/routines/lock`.
#[derive(Debug, Clone, Deserialize, PartialEq, Default)]
pub struct LockStatus {
    pub shared: bool,
    pub local: bool,
    pub locked: bool,
}

async fn api_lock_status() -> Result<LockStatus, String> {
    Request::get("/api/v1/routines/lock")
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json::<LockStatus>()
        .await
        .map_err(|e| e.to_string())
}

async fn api_unlock(scope: &str) -> Result<LockStatus, String> {
    let resp = Request::delete(&format!("/api/v1/routines/lock?scope={scope}"))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.ok() {
        return Err(format!("HTTP {}", resp.status()));
    }
    resp.json::<LockStatus>().await.map_err(|e| e.to_string())
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

async fn api_flags(id: &str) -> Result<Vec<Flag>, String> {
    let resp = Request::get(&format!("/api/v1/routines/{id}/flags"))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.ok() {
        return Err(format!("HTTP {}", resp.status()));
    }
    resp.json::<Vec<Flag>>().await.map_err(|e| e.to_string())
}

async fn api_resolve_flag(id: &str, filename: &str) -> Result<(), String> {
    let resp = Request::delete(&format!("/api/v1/routines/{id}/flags/{filename}"))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if resp.ok() {
        Ok(())
    } else {
        Err(format!("HTTP {}", resp.status()))
    }
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

/// Repository facet: all repositories, or one specific repository URL.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum RepositoryFacet {
    #[default]
    All,
    Named(String),
}

impl RepositoryFacet {
    const REPOSITORY_ALL: &'static str = "\u{0}all";

    #[must_use]
    pub fn as_value(&self) -> String {
        match self {
            RepositoryFacet::All => Self::REPOSITORY_ALL.to_string(),
            RepositoryFacet::Named(r) => r.clone(),
        }
    }

    #[must_use]
    pub fn from_value(v: &str) -> Self {
        if v == Self::REPOSITORY_ALL {
            RepositoryFacet::All
        } else {
            RepositoryFacet::Named(v.to_string())
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
    pub repository: RepositoryFacet,
}

impl RoutineFilter {
    /// `true` when at least one facet is narrowing the list.
    #[must_use]
    pub fn is_active(&self) -> bool {
        !self.query.trim().is_empty()
            || self.status != RoutineStatusFacet::All
            || self.agent != AgentFacet::All
            || self.machine != RoutineMachineFacet::Any
            || self.repository != RepositoryFacet::All
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
        match &self.repository {
            RepositoryFacet::All => {}
            RepositoryFacet::Named(rp) if !r.repositories.iter().any(|x| x.repository == *rp) => {
                return false
            }
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

/// Returns the most-recent trigger timestamp across both manual and scheduled fires.
/// `None` means the routine has never been triggered.
///
/// Uses the max of the two Optional timestamps so that whichever kind fired most
/// recently is what the LAST FIRE column shows.
pub(crate) fn last_fire_at(r: &Routine) -> Option<u64> {
    match (r.last_manual_trigger_at, r.last_scheduled_trigger_at) {
        (None, None) => None,
        (Some(m), None) => Some(m),
        (None, Some(s)) => Some(s),
        (Some(m), Some(s)) => Some(m.max(s)),
    }
}

// ─── Health status ────────────────────────────────────────────────────────────

/// At-a-glance operational health derived from a routine's current fields.
/// Covers the same fault categories as the Overview attention-reason triage
/// plus the `Disabled` state so every row in the Routines table has a badge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoutineHealth {
    /// Enabled but assigned to no machine — fires nowhere.
    Dormant,
    /// Enabled, has a machine, but the cron expression yields no future fire.
    DeadSchedule,
    /// Enabled, scheduled, has a machine, but agent config is missing.
    AgentMissing,
    /// `enabled: false` — intentionally paused.
    Disabled,
    /// Enabled, scheduled, agent registered, but the agent snoozed its own scheduled fires.
    Snoozed,
    /// Enabled, scheduled, has a machine, agent registered — fully operational.
    Healthy,
}

impl RoutineHealth {
    /// Lower number = more urgent. Ascending sort puts broken rows first.
    pub(crate) fn priority(self) -> u8 {
        match self {
            RoutineHealth::Dormant => 0,
            RoutineHealth::DeadSchedule => 1,
            RoutineHealth::AgentMissing => 2,
            RoutineHealth::Disabled => 3,
            RoutineHealth::Snoozed => 4,
            RoutineHealth::Healthy => 5,
        }
    }

    /// Short uppercase label shown in the badge.
    pub(crate) fn badge(self) -> &'static str {
        match self {
            RoutineHealth::Dormant => "DORMANT",
            RoutineHealth::DeadSchedule => "DEAD SCHEDULE",
            RoutineHealth::AgentMissing => "AGENT MISSING",
            RoutineHealth::Disabled => "DISABLED",
            RoutineHealth::Snoozed => "SNOOZED",
            RoutineHealth::Healthy => "HEALTHY",
        }
    }

    /// CSS class string for the badge `<span>`.
    pub(crate) fn badge_class(self) -> &'static str {
        match self {
            RoutineHealth::Dormant => "health-badge dormant",
            RoutineHealth::DeadSchedule => "health-badge dead",
            RoutineHealth::AgentMissing => "health-badge agent-missing",
            RoutineHealth::Disabled => "health-badge disabled",
            RoutineHealth::Snoozed => "health-badge snoozed",
            RoutineHealth::Healthy => "health-badge healthy",
        }
    }
}

/// Derive the operational health of a routine as of `now`.
///
/// Faults are checked in priority order — `Dormant` outranks `DeadSchedule`
/// which outranks `AgentMissing` — matching the Overview triage ordering.
#[must_use]
pub fn routine_health(r: &Routine, now: DateTime<Local>) -> RoutineHealth {
    if !r.enabled {
        return RoutineHealth::Disabled;
    }
    if r.machines.iter().all(|m| m.trim().is_empty()) {
        return RoutineHealth::Dormant;
    }
    if next_fire_after(&r.schedule, now).is_none() {
        return RoutineHealth::DeadSchedule;
    }
    if !r.agent_registered {
        return RoutineHealth::AgentMissing;
    }
    let snoozed = r
        .snoozed_until
        .is_some_and(|until| (until as i64) > now.timestamp())
        || r.skip_runs.is_some_and(|runs| runs > 0);
    if snoozed {
        return RoutineHealth::Snoozed;
    }
    RoutineHealth::Healthy
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

/// Distinct repository URLs across all routines, sorted.
#[must_use]
pub fn distinct_repositories(routines: &[Routine]) -> Vec<String> {
    let mut set: BTreeSet<String> = BTreeSet::new();
    for r in routines {
        for repo in &r.repositories {
            set.insert(repo.repository.clone());
        }
    }
    set.into_iter().collect()
}

// ─── State ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Default)]
pub enum RPage {
    #[default]
    List,
    New,
    Logs(String),
    Flags(String),
    /// Pre-filled create form cloned from an existing routine.
    Clone(Box<Routine>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum RModal {
    None,
    Edit(String),
    ConfirmDelete { id: String, title: String },
    ConfirmBulkDelete { count: usize },
}

/// How the list page presents routines: a table, or a month calendar of upcoming fire times.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum RView {
    #[default]
    Table,
    Calendar,
    Day,
}

/// Column the routine table is sorted by (click-to-sort).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RCol {
    Title,
    NextRun,
    LastFire,
    Agent,
    Health,
    Enabled,
    Updated,
}

/// Sort direction for the routine table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RDir {
    #[default]
    Asc,
    Desc,
}

impl RDir {
    /// Toggle to the opposite direction.
    #[must_use]
    pub fn flip(self) -> Self {
        match self {
            RDir::Asc => RDir::Desc,
            RDir::Desc => RDir::Asc,
        }
    }
}

// ─── Group-by ────────────────────────────────────────────────────────────────

/// Dimension used to partition the Routines table into labelled sections.
/// Orthogonal to faceted filtering and column sorting — composes with both.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RGroupBy {
    #[default]
    None,
    /// Group by the routine's agent (claude, codex, …).
    Agent,
    /// Group by target machine; routines with no machine share an `(unassigned)` section.
    Machine,
    /// Group by enabled/disabled status.
    Status,
}

impl RGroupBy {
    /// Stable token stored as the `<select>` option value.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            RGroupBy::None => "none",
            RGroupBy::Agent => "agent",
            RGroupBy::Machine => "machine",
            RGroupBy::Status => "status",
        }
    }

    /// Parse a token back to a variant, defaulting to `None` for unknown values.
    #[must_use]
    pub fn from_str(s: &str) -> Self {
        match s {
            "agent" => RGroupBy::Agent,
            "machine" => RGroupBy::Machine,
            "status" => RGroupBy::Status,
            _ => RGroupBy::None,
        }
    }

    /// Short human label shown in the selector dropdown.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            RGroupBy::None => "None",
            RGroupBy::Agent => "Agent",
            RGroupBy::Machine => "Machine",
            RGroupBy::Status => "Status",
        }
    }
}

/// Group key for a single routine under the given dimension.
#[must_use]
pub fn routine_group_key(r: &Routine, by: RGroupBy) -> String {
    match by {
        RGroupBy::None => String::new(),
        RGroupBy::Agent => r.agent.clone(),
        RGroupBy::Machine => r
            .machines
            .first()
            .cloned()
            .unwrap_or_else(|| "(unassigned)".to_string()),
        RGroupBy::Status => {
            if r.enabled {
                "Enabled".to_string()
            } else {
                "Disabled".to_string()
            }
        }
    }
}

/// Partition `routines` into `(group_label, routines_in_group)` pairs sorted
/// alphabetically by label. Within each group the input order is preserved.
/// When `by` is `None`, returns a single pair with an empty label.
#[must_use]
pub fn group_routines(routines: &[Routine], by: RGroupBy) -> Vec<(String, Vec<Routine>)> {
    use std::collections::BTreeMap;
    if by == RGroupBy::None {
        return vec![(String::new(), routines.to_vec())];
    }
    let mut map: BTreeMap<String, Vec<Routine>> = BTreeMap::new();
    for r in routines {
        map.entry(routine_group_key(r, by))
            .or_default()
            .push(r.clone());
    }
    map.into_iter().collect()
}

/// Return `routines` sorted by `col` in `dir` order. When `col` is `None` the
/// server/insertion order is preserved. Ties break by id for a stable sort.
#[must_use]
pub fn sort_routines(
    mut routines: Vec<Routine>,
    col: Option<RCol>,
    dir: RDir,
    now: DateTime<Local>,
) -> Vec<Routine> {
    let col = match col {
        Some(c) => c,
        None => return routines,
    };
    routines.sort_by(|a, b| {
        let primary = match col {
            RCol::Title => a.title.to_lowercase().cmp(&b.title.to_lowercase()),
            RCol::Agent => a.agent.to_lowercase().cmp(&b.agent.to_lowercase()),
            RCol::Enabled => a.enabled.cmp(&b.enabled),
            RCol::Updated => a.updated_at.cmp(&b.updated_at),
            RCol::Health => routine_health(a, now)
                .priority()
                .cmp(&routine_health(b, now).priority()),
            RCol::LastFire => last_fire_at(a).cmp(&last_fire_at(b)),
            RCol::NextRun => {
                let next_of = |r: &Routine| {
                    if r.enabled {
                        next_fire_after(&r.schedule, now)
                    } else {
                        None
                    }
                };
                match (next_of(a), next_of(b)) {
                    (Some(ta), Some(tb)) => ta.cmp(&tb),
                    (Some(_), None) => std::cmp::Ordering::Less,
                    (None, Some(_)) => std::cmp::Ordering::Greater,
                    (None, None) => std::cmp::Ordering::Equal,
                }
            }
        };
        let directed = if dir == RDir::Desc {
            primary.reverse()
        } else {
            primary
        };
        directed.then_with(|| a.id.cmp(&b.id))
    });
    routines
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
    /// Column the table is sorted by (`None` = natural order).
    pub sort_col: Option<RCol>,
    /// Direction of the active column sort.
    pub sort_dir: RDir,
    /// IDs of currently selected routines (multiselect for bulk actions).
    pub selected: BTreeSet<String>,
    /// Active group-by dimension; `None` renders a flat list.
    pub group_by: RGroupBy,
    /// Most recently fetched global lock status; `None` until the first fetch completes.
    pub lock_status: Option<LockStatus>,
    /// This machine's resolved name from the daemon, used to default the machine facet.
    pub current_machine: Option<String>,
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
            sort_col: None,
            sort_dir: RDir::default(),
            selected: BTreeSet::new(),
            group_by: RGroupBy::default(),
            lock_status: None,
            current_machine: None,
        }
    }
}

pub enum RAction {
    Loaded(Vec<Routine>),
    GoToNew,
    GoToList,
    GoToLogs(String),
    GoToFlags(String),
    /// Open the create form pre-filled with a copy of the named routine.
    GoToClone(String),
    OpenEdit(String),
    OpenConfirmDelete {
        id: String,
        title: String,
    },
    OpenConfirmBulkDelete,
    CloseModal,
    SetView(RView),
    SetQuery(String),
    SetStatusFacet(RoutineStatusFacet),
    SetAgentFacet(AgentFacet),
    SetMachineFacet(RoutineMachineFacet),
    SetRepositoryFacet(RepositoryFacet),
    ClearFilters,
    /// Change the group-by dimension for the table view.
    SetGroupBy(RGroupBy),
    SortByCol(RCol),
    Upsert(Box<Routine>),
    Remove(String),
    /// Remove multiple routines after a confirmed bulk delete.
    RemoveMany(Vec<String>),
    /// Toggle one routine in/out of the selection set.
    SelectRoutine(String),
    /// Select exactly the given (visible/filtered) routine ids.
    SelectAll(Vec<String>),
    /// Clear the entire selection.
    ClearSelection,
    /// Received updated lock status from the server.
    LockStatusLoaded(LockStatus),
    /// Resolved current machine name received from the daemon; defaults machine facet to it.
    CurrentMachineLoaded(String),
}

impl Reducible for RState {
    type Action = RAction;

    fn reduce(self: Rc<Self>, action: Self::Action) -> Rc<Self> {
        let mut s = (*self).clone();
        match action {
            RAction::Loaded(r) => {
                // Drop selections for routines that no longer exist after a reload.
                let ids: BTreeSet<&String> = r.iter().map(|x| &x.id).collect();
                s.selected.retain(|id| ids.contains(id));
                s.routines = r;
                s.loading = false;
            }
            RAction::GoToNew => s.page = RPage::New,
            RAction::GoToList => s.page = RPage::List,
            RAction::GoToLogs(id) => s.page = RPage::Logs(id),
            RAction::GoToFlags(id) => s.page = RPage::Flags(id),
            RAction::GoToClone(id) => {
                if let Some(source) = s.routines.iter().find(|x| x.id == id) {
                    s.page = RPage::Clone(Box::new(source.clone()));
                }
            }
            RAction::OpenEdit(id) => s.modal = RModal::Edit(id),
            RAction::OpenConfirmDelete { id, title } => {
                s.modal = RModal::ConfirmDelete { id, title }
            }
            RAction::OpenConfirmBulkDelete => {
                s.modal = RModal::ConfirmBulkDelete {
                    count: s.selected.len(),
                };
            }
            RAction::CloseModal => s.modal = RModal::None,
            RAction::SetView(view) => s.view = view,
            RAction::SetQuery(q) => s.filter.query = q,
            RAction::SetStatusFacet(st) => s.filter.status = st,
            RAction::SetAgentFacet(ag) => s.filter.agent = ag,
            RAction::SetMachineFacet(m) => s.filter.machine = m,
            RAction::SetRepositoryFacet(rp) => s.filter.repository = rp,
            RAction::ClearFilters => s.filter = RoutineFilter::default(),
            RAction::SetGroupBy(by) => s.group_by = by,
            RAction::SortByCol(col) => {
                if s.sort_col == Some(col) {
                    s.sort_dir = s.sort_dir.flip();
                } else {
                    s.sort_col = Some(col);
                    s.sort_dir = RDir::Asc;
                }
            }
            RAction::Upsert(routine) => {
                let routine = *routine;
                if let Some(i) = s.routines.iter().position(|x| x.id == routine.id) {
                    s.routines[i] = routine;
                } else {
                    s.routines.push(routine);
                }
            }
            RAction::Remove(id) => {
                s.routines.retain(|x| x.id != id);
                s.selected.remove(&id);
            }
            RAction::RemoveMany(ids) => {
                let drop: BTreeSet<&String> = ids.iter().collect();
                s.routines.retain(|r| !drop.contains(&r.id));
                s.selected.retain(|id| !drop.contains(id));
            }
            RAction::SelectRoutine(id) => {
                if !s.selected.remove(&id) {
                    s.selected.insert(id);
                }
            }
            RAction::SelectAll(ids) => {
                s.selected = ids.into_iter().collect();
            }
            RAction::ClearSelection => {
                s.selected.clear();
            }
            RAction::LockStatusLoaded(status) => {
                s.lock_status = Some(status);
            }
            RAction::CurrentMachineLoaded(name) => {
                s.current_machine = Some(name.clone());
                s.filter.machine = RoutineMachineFacet::Machine(name);
            }
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

    // Fetch and apply the current machine as the default machine filter.
    {
        let state = state.clone();
        use_effect_with((), move |_| {
            spawn_local(async move {
                if let Ok(name) = api_current_machine().await {
                    state.dispatch(RAction::CurrentMachineLoaded(name));
                }
            });
        });
    }

    // Fetch lock status on mount and whenever routines reload.
    {
        let state = state.clone();
        use_effect_with((), move |_| {
            spawn_local(async move {
                if let Ok(status) = api_lock_status().await {
                    state.dispatch(RAction::LockStatusLoaded(status));
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

    let on_unlock_all = {
        let state = state.clone();
        let toast = toast.clone();
        let ok = ok_toast.clone();
        Callback::from(move |_: MouseEvent| {
            let state = state.clone();
            let toast = toast.clone();
            let ok = ok.clone();
            spawn_local(async move {
                match api_unlock("all").await {
                    Ok(status) => {
                        state.dispatch(RAction::LockStatusLoaded(status));
                        ok("Routines unlocked");
                    }
                    Err(err_msg) => {
                        toast.emit((format!("Unlock failed: {err_msg}"), ToastKind::Err))
                    }
                }
            })
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
    let on_flags = {
        let state = state.clone();
        Callback::from(move |id: String| state.dispatch(RAction::GoToFlags(id)))
    };
    let on_back = {
        let state = state.clone();
        Callback::from(move |_: ()| state.dispatch(RAction::GoToList))
    };
    let on_edit = {
        let state = state.clone();
        Callback::from(move |id: String| state.dispatch(RAction::OpenEdit(id)))
    };
    let on_clone = {
        let state = state.clone();
        Callback::from(move |id: String| state.dispatch(RAction::GoToClone(id)))
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
    let on_set_group_by = {
        let state = state.clone();
        Callback::from(move |by: RGroupBy| state.dispatch(RAction::SetGroupBy(by)))
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
    let on_set_repository = {
        let state = state.clone();
        Callback::from(move |rp: RepositoryFacet| state.dispatch(RAction::SetRepositoryFacet(rp)))
    };
    let on_clear_filters = {
        let state = state.clone();
        Callback::from(move |_: ()| state.dispatch(RAction::ClearFilters))
    };

    // `/` focuses the search box (while not already typing in another field),
    // matching the GitHub/Slack convention and complementing the ⌘K palette.
    // Escape dismisses whichever routine modal/dialog is currently open.
    let search_ref = use_node_ref();
    {
        let search_ref = search_ref.clone();
        let state = state.clone();
        use_effect_with((), move |_| {
            let on_key =
                Closure::<dyn Fn(KeyboardEvent)>::wrap(Box::new(move |event: KeyboardEvent| {
                    if event.key() == "Escape" {
                        if state.modal != RModal::None {
                            state.dispatch(RAction::CloseModal);
                        }
                        return;
                    }
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

    let on_col_sort = {
        let state = state.clone();
        Callback::from(move |col: RCol| state.dispatch(RAction::SortByCol(col)))
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
                        tags: Some(req.tags),
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

    // ── Bulk selection ────────────────────────────────────────────────────────
    let on_select = {
        let state = state.clone();
        Callback::from(move |id: String| state.dispatch(RAction::SelectRoutine(id)))
    };

    // Header checkbox: toggle "all visible selected ↔ none" (filter-scoped).
    let on_select_all = {
        let state = state.clone();
        let now = now.clone();
        Callback::from(move |_: ()| {
            let window = Duration::seconds(DUE_SOON_WINDOW_SECS);
            let visible = filter_routines(&state.routines, &state.filter, *now, window);
            let all_visible_selected =
                !visible.is_empty() && visible.iter().all(|r| state.selected.contains(&r.id));
            if all_visible_selected {
                state.dispatch(RAction::ClearSelection);
            } else {
                state.dispatch(RAction::SelectAll(
                    visible.into_iter().map(|r| r.id).collect(),
                ));
            }
        })
    };

    let on_clear_selection = {
        let state = state.clone();
        Callback::from(move |_: ()| state.dispatch(RAction::ClearSelection))
    };

    // Bulk enable/disable: PATCH each selected routine, surface one summary toast.
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
                    let req = UpdateRoutineRequest {
                        enabled: Some(enabled),
                        ..Default::default()
                    };
                    match api_update(&id, &req).await {
                        Ok(r) => {
                            state.dispatch(RAction::Upsert(Box::new(r)));
                            ok += 1;
                        }
                        Err(_) => failed += 1,
                    }
                }
                let verb = if enabled { "enabled" } else { "disabled" };
                if failed == 0 {
                    toast.emit((format!("{ok} routine(s) {verb}"), ToastKind::Ok));
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
        Callback::from(move |_: ()| state.dispatch(RAction::OpenConfirmBulkDelete))
    };

    let on_confirm_bulk_delete = {
        let state = state.clone();
        let toast = toast.clone();
        Callback::from(move |_: ()| {
            let state = state.clone();
            let toast = toast.clone();
            let ids: Vec<String> = state.selected.iter().cloned().collect();
            spawn_local(async move {
                let mut ok = 0usize;
                let mut failed = 0usize;
                let mut deleted: Vec<String> = Vec::new();
                for id in ids {
                    match api_delete(&id).await {
                        Ok(()) => {
                            deleted.push(id);
                            ok += 1;
                        }
                        Err(_) => failed += 1,
                    }
                }
                state.dispatch(RAction::RemoveMany(deleted));
                state.dispatch(RAction::CloseModal);
                if failed == 0 {
                    toast.emit((format!("{ok} routine(s) deleted"), ToastKind::Ok));
                } else {
                    toast.emit((format!("{ok} deleted, {failed} failed"), ToastKind::Err));
                }
            });
        })
    };

    let routines = state.routines.clone();
    let loading = state.loading;
    let page = state.page.clone();
    let modal = state.modal.clone();
    let lock_status = state.lock_status.clone();
    let view = state.view;
    let filter = state.filter.clone();
    let sort_col = state.sort_col;
    let sort_dir = state.sort_dir;
    let selected = state.selected.clone();
    let group_by = state.group_by;

    // Faceted filter + sort applied client-side.
    let now_val = *now;
    let window = Duration::seconds(DUE_SOON_WINDOW_SECS);
    let total_routines = routines.len();
    let agent_options = distinct_agents(&routines);
    let repository_options = distinct_repositories(&routines);
    let mut machine_options = distinct_machines_r(&routines);
    // Always include the current machine so the default filter option is visible in the dropdown
    // even before any routine targets it.
    if let Some(cm) = &state.current_machine {
        if !machine_options.contains(cm) {
            machine_options.push(cm.clone());
            machine_options.sort();
        }
    }
    let filter_active = filter.is_active();
    let visible = {
        let filtered = filter_routines(&routines, &filter, now_val, window);
        sort_routines(filtered, sort_col, sort_dir, now_val)
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
                    RPage::Clone(source) => {
                        let mut pre = *source;
                        pre.title = clone_title(&pre.title);
                        html! {
                            <RoutineForm editing={Some(pre)} on_cancel={on_cancel} on_save={on_create} />
                        }
                    },
                    RPage::Logs(id) => {
                        let title = routines.iter()
                            .find(|r| r.id == id)
                            .map(|r| r.title.clone())
                            .unwrap_or_default();
                        html! { <RoutineLogs id={id} title={title} on_back={on_back} /> }
                    },
                    RPage::Flags(id) => {
                        let title = routines.iter()
                            .find(|r| r.id == id)
                            .map(|r| r.title.clone())
                            .unwrap_or_default();
                        html! { <RoutineFlags id={id} title={title} on_back={on_back} /> }
                    },
                    RPage::List => html! {
                        <main>
                            <GlobalLockBanner status={lock_status} on_unlock={on_unlock_all} />
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
                                    if view == RView::Table {
                                        <RoutineGroupBySelector
                                            group_by={group_by}
                                            on_change={on_set_group_by}
                                        />
                                    }
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
                                repositories={repository_options}
                                shown={shown}
                                total={total_routines}
                                search_ref={search_ref.clone()}
                                on_query={on_set_query}
                                on_status={on_set_status}
                                on_agent={on_set_agent}
                                on_machine={on_set_machine}
                                on_repository={on_set_repository}
                                on_clear={on_clear_filters.clone()}
                            />
                            <RoutineBulkBar
                                count={selected.len()}
                                on_enable={on_bulk_enable}
                                on_disable={on_bulk_disable}
                                on_delete={on_bulk_delete}
                                on_clear={on_clear_selection}
                            />
                            {
                                match view {
                                    RView::Table => html! {
                                        <RoutineTable
                                            routines={visible}
                                            loading={loading}
                                            filter_active={filter_active}
                                            now={now_val}
                                            selected={selected.clone()}
                                            on_select={on_select}
                                            on_select_all={on_select_all}
                                            sort_col={sort_col}
                                            sort_dir={sort_dir}
                                            group_by={group_by}
                                            on_sort={on_col_sort}
                                            on_edit={on_edit}
                                            on_clone={on_clone}
                                            on_delete={on_ask_delete}
                                            on_toggle={on_toggle}
                                            on_trigger={on_trigger}
                                            on_logs={on_logs}
                                            on_flags={on_flags}
                                            on_clear_filters={on_clear_filters}
                                        />
                                    },
                                    RView::Calendar => html! {
                                        <RoutineCalendar routines={visible} loading={loading} on_edit={Some(on_edit)} on_toast={Some(toast.clone())} />
                                    },
                                    RView::Day => {
                                        let items = visible.iter().filter(|r| r.enabled).map(|r| TimelineItem {
                                            id: Some(r.id.clone()),
                                            label: r.title.clone(),
                                            schedule: r.schedule.clone(),
                                        }).collect::<Vec<_>>();
                                        html! { <DayTimeline items={items} loading={loading} on_click={Some(on_edit)} /> }
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
                    RModal::ConfirmBulkDelete { count } => html! {
                        <RoutineBulkDeleteDialog
                            count={*count}
                            on_cancel={on_close}
                            on_confirm={on_confirm_bulk_delete}
                        />
                    },
                    RModal::None => html! {},
                }
            }
        </>
    }
}

// ─── Global lock banner ───────────────────────────────────────────────────────

#[derive(Properties, PartialEq)]
pub struct GlobalLockBannerProps {
    /// Current lock status; `None` hides the banner (status not yet fetched).
    pub status: Option<LockStatus>,
    /// Called when the user clicks UNLOCK ALL.
    pub on_unlock: Callback<MouseEvent>,
}

/// Banner shown above the routine list when the global lock is active.
///
/// Displays which sentinel(s) are present (SHARED / LOCAL) and an UNLOCK ALL button
/// that removes both with `DELETE /api/v1/routines/lock?scope=all`.
#[function_component(GlobalLockBanner)]
pub fn global_lock_banner(props: &GlobalLockBannerProps) -> Html {
    let Some(ref status) = props.status else {
        return html! {};
    };
    if !status.locked {
        return html! {};
    }
    html! {
        <div class="lock-banner">
            <div class="lock-banner-msg">
                {"⚠ ROUTINES GLOBALLY LOCKED — scheduling and manual triggers paused"}
                if status.shared {
                    <span class="lock-scope-tag">{"SHARED .lock"}</span>
                }
                if status.local {
                    <span class="lock-scope-tag">{"LOCAL .local.lock"}</span>
                }
            </div>
            <div class="lock-banner-acts">
                <button class="btn btn-ghost btn-sm" onclick={props.on_unlock.clone()}>
                    {"UNLOCK ALL"}
                </button>
            </div>
        </div>
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
/// the matching status facet on the list below.
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

// ─── Group-by selector ────────────────────────────────────────────────────────

#[derive(Properties, PartialEq)]
pub struct GroupBySelectorProps {
    pub group_by: RGroupBy,
    pub on_change: Callback<RGroupBy>,
}

/// Drop-down that lets the operator choose how to partition the Routines table.
/// Placed in the section toolbar next to the view toggle; hidden for Calendar/Day views.
#[function_component(RoutineGroupBySelector)]
pub fn routine_group_by_selector(props: &GroupBySelectorProps) -> Html {
    let on_change = props.on_change.clone();
    let on_select = Callback::from(move |e: Event| {
        let select: HtmlSelectElement = e.target_unchecked_into();
        on_change.emit(RGroupBy::from_str(&select.value()));
    });
    let cur = props.group_by.as_str();
    html! {
        <div class="group-by-ctrl">
            <label class="filter-label" for="routine-group-by-select">{"GROUP BY"}</label>
            <select
                id="routine-group-by-select"
                class="filter-select"
                aria-label="Group routines by"
                onchange={on_select}
            >
                { for [RGroupBy::None, RGroupBy::Agent, RGroupBy::Machine, RGroupBy::Status].iter()
                    .map(|&by| html! {
                        <option value={by.as_str()} selected={cur == by.as_str()}>
                            { by.label() }
                        </option>
                    })
                }
            </select>
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
    /// Distinct repository URLs across all routines, for the repository-facet options.
    pub repositories: Vec<String>,
    /// Count after filtering / total loaded — rendered as "Showing N of M".
    pub shown: usize,
    pub total: usize,
    /// NodeRef forwarded from the page so the `/` shortcut can focus this input.
    pub search_ref: NodeRef,
    pub on_query: Callback<String>,
    pub on_status: Callback<RoutineStatusFacet>,
    pub on_agent: Callback<AgentFacet>,
    pub on_machine: Callback<RoutineMachineFacet>,
    pub on_repository: Callback<RepositoryFacet>,
    pub on_clear: Callback<()>,
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
    let on_repository_change = {
        let cb = props.on_repository.clone();
        Callback::from(move |e: Event| {
            let select: HtmlSelectElement = e.target_unchecked_into();
            cb.emit(RepositoryFacet::from_value(&select.value()));
        })
    };
    let on_clear = {
        let cb = props.on_clear.clone();
        Callback::from(move |_: MouseEvent| cb.emit(()))
    };
    let status_val = props.filter.status.as_str();
    let agent_val = props.filter.agent.as_value();
    let machine_val = props.filter.machine.as_value();
    let repository_val = props.filter.repository.as_value();
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
                    <option value={RMACHINE_UNASSIGNED}
                        selected={machine_val == RMACHINE_UNASSIGNED}>{"None"}</option>
                    { for props.machines.iter().map(|m| html! {
                        <option value={m.clone()} selected={machine_val == *m}>{m.clone()}</option>
                    }) }
                </select>
                <span class="filter-label">{"REPOSITORY"}</span>
                <select class="filter-select" aria-label="Repository filter" onchange={on_repository_change}>
                    <option value={RepositoryFacet::REPOSITORY_ALL}
                        selected={repository_val == RepositoryFacet::REPOSITORY_ALL}>{"Any"}</option>
                    { for props.repositories.iter().map(|r| html! {
                        <option value={r.clone()} selected={repository_val == *r}>{r.clone()}</option>
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

// ─── Calendar ─────────────────────────────────────────────────────────────────

#[derive(Properties, PartialEq)]
pub struct CalendarProps {
    pub routines: Vec<Routine>,
    pub loading: bool,
    /// When set, clicking a calendar chip opens the edit modal for that routine.
    #[prop_or_default]
    pub on_edit: Option<Callback<String>>,
    /// When set, enables the SUBSCRIBE button which copies the `/routines.ics` feed URL.
    #[prop_or_default]
    pub on_toast: Option<Callback<(String, ToastKind)>>,
}

/// Build the absolute URL of the routines iCalendar feed from a page origin.
fn ics_feed_url(origin: &str) -> String {
    format!("{origin}/api/v1/routines.ics")
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
    let on_subscribe = props.on_toast.clone().map(|on_toast| {
        Callback::from(move |_: MouseEvent| {
            let on_toast = on_toast.clone();
            spawn_local(async move {
                let Some(window) = web_sys::window() else {
                    return;
                };
                let origin = window.location().origin().unwrap_or_default();
                let url = ics_feed_url(&origin);
                let promise = window.navigator().clipboard().write_text(&url);
                match wasm_bindgen_futures::JsFuture::from(promise).await {
                    Ok(_) => on_toast.emit(("Calendar feed URL copied".into(), ToastKind::Ok)),
                    Err(_) => on_toast.emit(("Copy failed".into(), ToastKind::Err)),
                }
            });
        })
    });

    if props.loading {
        return html! {
            <div class="table-wrap"><div class="empty"><div class="spinner"></div></div></div>
        };
    }

    let today = Local::now().date_naive();
    let first = month_start(today, *offset);
    let grid_start = first - Duration::days(first.weekday().num_days_from_sunday() as i64);

    // Accumulate per-cell chips in routine order: only enabled routines with a parseable schedule.
    // Each entry is (id, title, count) so chips can dispatch the edit modal on click.
    let mut cells: Vec<Vec<(String, String, u32)>> = vec![Vec::new(); GRID_CELLS];
    let mut scheduled = 0usize;
    for r in props.routines.iter().filter(|r| r.enabled) {
        if let Some(counts) = occurrences_per_day(&r.schedule, grid_start) {
            scheduled += 1;
            for (i, &c) in counts.iter().enumerate() {
                if c > 0 {
                    cells[i].push((r.id.clone(), r.title.clone(), c));
                }
            }
        }
    }

    let month_label = format!("{} {}", CAL_MONTHS[first.month0() as usize], first.year());

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
                                    { for hits.iter().take(4).map(|(id, title, count)| {
                                        let label = if *count > 1 {
                                            format!("{title} ×{count}")
                                        } else {
                                            title.clone()
                                        };
                                        let on_chip = props.on_edit.as_ref().map(|cb| {
                                            let cb = cb.clone();
                                            let id = id.clone();
                                            Callback::from(move |_: MouseEvent| cb.emit(id.clone()))
                                        });
                                        let chip_cls = if on_chip.is_some() { "cal-chip clickable" } else { "cal-chip" };
                                        html! { <div class={chip_cls} title={label.clone()} onclick={on_chip}>{label}</div> }
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
                if let Some(on_subscribe) = on_subscribe {
                    <button class="btn btn-ghost btn-sm" title="Copy the routines.ics feed URL"
                        onclick={on_subscribe}>{"SUBSCRIBE"}</button>
                }
            </div>
            {body}
        </div>
    }
}

// ─── Table ────────────────────────────────────────────────────────────────────

/// Render a sortable `<th>` cell. Active column shows ▲ / ▼; inactive columns
/// are plain clickable headers. Clicking the active column reverses direction.
fn sort_th(
    label: &'static str,
    col: RCol,
    current: Option<RCol>,
    dir: RDir,
    on_sort: &Callback<RCol>,
) -> Html {
    let active = current == Some(col);
    let indicator = if active {
        if dir == RDir::Asc {
            " ▲"
        } else {
            " ▼"
        }
    } else {
        ""
    };
    let cls = if active {
        "th-sort th-sort-active"
    } else {
        "th-sort"
    };
    let cb = on_sort.clone();
    html! {
        <th class={cls} onclick={Callback::from(move |_: MouseEvent| cb.emit(col))}>
            { format!("{label}{indicator}") }
        </th>
    }
}

#[derive(Properties, PartialEq)]
pub struct TableProps {
    pub routines: Vec<Routine>,
    pub loading: bool,
    /// Whether a filter is narrowing the list — selects the filtered-empty state.
    pub filter_active: bool,
    /// Reference instant used to compute next-fire countdowns.
    pub now: chrono::DateTime<Local>,
    /// Set of currently selected routine IDs.
    pub selected: BTreeSet<String>,
    /// Fired when the user clicks a row's selection checkbox.
    pub on_select: Callback<String>,
    /// Fired when the header checkbox is clicked (toggles all-visible).
    pub on_select_all: Callback<()>,
    /// Active sort column (`None` = natural order).
    pub sort_col: Option<RCol>,
    /// Direction of the active column sort.
    pub sort_dir: RDir,
    /// Active group-by dimension; `None` renders a flat list.
    pub group_by: RGroupBy,
    /// Fired when the user clicks a sortable column header.
    pub on_sort: Callback<RCol>,
    pub on_edit: Callback<String>,
    pub on_clone: Callback<String>,
    pub on_delete: Callback<(String, String)>,
    pub on_toggle: Callback<(String, bool)>,
    pub on_trigger: Callback<String>,
    pub on_logs: Callback<String>,
    pub on_flags: Callback<String>,
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

    let all_visible_selected = !props.routines.is_empty()
        && props
            .routines
            .iter()
            .all(|r| props.selected.contains(&r.id));
    let on_select_all = {
        let cb = props.on_select_all.clone();
        Callback::from(move |_: MouseEvent| cb.emit(()))
    };

    html! {
        <div class="table-wrap">
            <table>
                <thead>
                    <tr>
                        <th class="col-select">
                            <input
                                type="checkbox"
                                checked={all_visible_selected}
                                onclick={on_select_all}
                                aria-label="Select all visible routines"
                                title="Select all visible"
                            />
                        </th>
                        { sort_th("TITLE", RCol::Title, props.sort_col, props.sort_dir, &props.on_sort) }
                        <th>{"SCHEDULE"}</th>
                        { sort_th("NEXT RUN", RCol::NextRun, props.sort_col, props.sort_dir, &props.on_sort) }
                        { sort_th("LAST FIRE", RCol::LastFire, props.sort_col, props.sort_dir, &props.on_sort) }
                        { sort_th("AGENT", RCol::Agent, props.sort_col, props.sort_dir, &props.on_sort) }
                        <th>{"REPOS"}</th>
                        <th>{"TAGS"}</th>
                        <th>{"TTL"}</th>
                        { sort_th("HEALTH", RCol::Health, props.sort_col, props.sort_dir, &props.on_sort) }
                        { sort_th("ENABLED", RCol::Enabled, props.sort_col, props.sort_dir, &props.on_sort) }
                        { sort_th("UPDATED", RCol::Updated, props.sort_col, props.sort_dir, &props.on_sort) }
                        <th></th>
                    </tr>
                </thead>
                <tbody>
                    { for group_routines(&props.routines, props.group_by).into_iter().map(|(label, group)| {
                        let count = group.len();
                        let grouped = props.group_by != RGroupBy::None;
                        html! {
                            <>
                                if grouped {
                                    <tr class="group-hd" key={format!("ghd-{label}")}>
                                        <td colspan="12">
                                            <span class="group-label">{label.clone()}</span>
                                            <span class="group-count">{format!("({count})")}</span>
                                        </td>
                                    </tr>
                                }
                                { for group.into_iter().map(|r| html! {
                                    <RoutineRow
                                        key={r.id.clone()}
                                        routine={r.clone()}
                                        now={props.now}
                                        selected={props.selected.contains(&r.id)}
                                        on_select={props.on_select.clone()}
                                        on_edit={props.on_edit.clone()}
                                        on_clone={props.on_clone.clone()}
                                        on_delete={props.on_delete.clone()}
                                        on_toggle={props.on_toggle.clone()}
                                        on_trigger={props.on_trigger.clone()}
                                        on_logs={props.on_logs.clone()}
                                        on_flags={props.on_flags.clone()}
                                    />
                                }) }
                            </>
                        }
                    }) }
                </tbody>
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
    /// Whether this row is currently selected.
    pub selected: bool,
    /// Fired when the selection checkbox is clicked.
    pub on_select: Callback<String>,
    pub on_edit: Callback<String>,
    pub on_clone: Callback<String>,
    pub on_delete: Callback<(String, String)>,
    pub on_toggle: Callback<(String, bool)>,
    pub on_trigger: Callback<String>,
    pub on_logs: Callback<String>,
    pub on_flags: Callback<String>,
}

#[function_component(RoutineRow)]
pub fn routine_row(props: &RowProps) -> Html {
    let preview_open = use_state(|| false);

    let r = &props.routine;
    let cron_text = r.schedule_description.as_deref().unwrap_or("—").to_string();
    let updated = reltime(r.updated_at);
    let repos = r.repositories.len();

    let on_edit = {
        let cb = props.on_edit.clone();
        let id = r.id.clone();
        Callback::from(move |_: MouseEvent| cb.emit(id.clone()))
    };
    let on_clone = {
        let cb = props.on_clone.clone();
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
    let on_flags = {
        let cb = props.on_flags.clone();
        let id = r.id.clone();
        Callback::from(move |_: MouseEvent| cb.emit(id.clone()))
    };

    let on_select = {
        let cb = props.on_select.clone();
        let id = r.id.clone();
        Callback::from(move |_: MouseEvent| cb.emit(id.clone()))
    };

    let on_preview_toggle = {
        let preview_open = preview_open.clone();
        Callback::from(move |e: MouseEvent| {
            e.stop_propagation();
            preview_open.set(!*preview_open);
        })
    };

    let fires_panel = if *preview_open {
        let fires = next_fires(&r.schedule, props.now, 10);
        if fires.is_empty() {
            html! { <div class="fires-panel"><div class="fires-empty">{"— no future fires —"}</div></div> }
        } else {
            let now = props.now;
            html! {
                <div class="fires-panel">
                    <div class="fires-hd">{"NEXT 10 FIRES"}</div>
                    { for fires.iter().map(|t| html! {
                        <div class="fires-item">
                            <span class="fires-when">{fmt_when(now, *t)}</span>
                            <span class="fires-until">{fmt_until(now, *t)}</span>
                        </div>
                    }) }
                </div>
            }
        }
    } else {
        html! {}
    };

    let preview_btn_class = if *preview_open {
        "sched-preview-btn open"
    } else {
        "sched-preview-btn"
    };

    // LAST FIRE: most-recent trigger timestamp from last_fire_at, icon chosen by type.
    // ↻ = the most-recent fire was a manual trigger; ⏱ = it was a scheduled fire.
    let last_fire: Html = match last_fire_at(r) {
        None => html! { <span class="muted">{"—"}</span> },
        Some(ts) => {
            let manual = r.last_manual_trigger_at;
            let scheduled = r.last_scheduled_trigger_at;
            let icon = if manual.is_some_and(|m| scheduled.is_none_or(|s| m >= s)) {
                "↻"
            } else {
                "⏱"
            };
            html! { <div class="cell-triggered">{format!("{icon} {}", reltime(ts))}</div> }
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
        <tr class={if props.selected { "row-selected" } else { "" }}>
            <td class="col-select">
                <input type="checkbox" checked={props.selected} onclick={on_select}
                    aria-label={format!("Select {}", r.title)} />
            </td>
            <td>
                <div class="cell-schedule" title={r.id.clone()}>{&r.title}</div>
            </td>
            <td>
                <div class="cell-schedule">{&r.schedule}</div>
                <div class="cell-schedule-human">{cron_text}</div>
                <button
                    class={preview_btn_class}
                    title="Preview next fire times"
                    aria-label="Preview next scheduled fire times"
                    aria-expanded={(*preview_open).to_string()}
                    onclick={on_preview_toggle}
                >{"▸ fires"}</button>
                {fires_panel}
            </td>
            <td>{next_run}</td>
            <td>{last_fire}</td>
            <td>
                <span class="cell-handler" title={agent_title}>
                    <span class={agent_dot}></span>
                    {&r.agent}
                </span>
            </td>
            <td><span class="cell-meta">{ if repos == 0 { "—".to_string() } else { format!("{repos}") } }</span></td>
            <td>
                {
                    if r.tags.is_empty() {
                        html! { <span class="cell-meta">{"—"}</span> }
                    } else {
                        html! {
                            <span class="cell-meta" title={r.tags.join(", ")}>{ r.tags.join(", ") }</span>
                        }
                    }
                }
            </td>
            <td><span class="cell-meta" title="workbench retention for finished runs">{ format_ttl(r.ttl_secs) }</span></td>
            <td>
                <span class={routine_health(r, props.now).badge_class()}
                    title={routine_health(r, props.now).badge()}>
                    {routine_health(r, props.now).badge()}
                </span>
            </td>
            <td>
                <label class="toggle">
                    <input type="checkbox" checked={r.enabled} onchange={on_toggle} />
                    <div class="toggle-track"></div>
                </label>
            </td>
            <td><div class="cell-time">{updated}</div></td>
            <td>
                <div class="row-actions">
                    <button class="act-btn run" title="Run now" aria-label="Run now" onclick={on_trigger}>{"▶"}</button>
                    <button class="act-btn logs" onclick={on_logs}>{"LOGS"}</button>
                    <button class="act-btn flags" title="Open flags" onclick={on_flags}>
                        {"FLAGS"}
                        if r.flag_count > 0 {
                            <span class="flag-badge">{r.flag_count}</span>
                        }
                    </button>
                    <button class="act-btn edit" onclick={on_edit}>{"EDIT"}</button>
                    <button class="act-btn clone" title="Duplicate routine" aria-label="Duplicate routine" onclick={on_clone}>{"⧉"}</button>
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

/// Title for a cloned routine: prepend "Copy of " when the original title does not
/// already start with that prefix, preventing "Copy of Copy of …" accumulation.
pub(crate) fn clone_title(title: &str) -> String {
    const PREFIX: &str = "Copy of ";
    if title.starts_with(PREFIX) {
        title.to_string()
    } else {
        format!("{PREFIX}{title}")
    }
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

/// Join tags into a single comma-separated string for the input field.
fn tags_to_text(tags: &[String]) -> String {
    tags.join(", ")
}

/// Split a comma-separated input into trimmed, non-empty tags.
fn text_to_tags(text: &str) -> Vec<String> {
    text.split(',')
        .map(str::trim)
        .filter(|tag| !tag.is_empty())
        .map(str::to_string)
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
    // Comma-separated tags; blank means no tags.
    let tags_raw = use_state(|| {
        editing
            .as_ref()
            .map(|r| tags_to_text(&r.tags))
            .unwrap_or_default()
    });
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
    let on_tags = {
        let tags_raw = tags_raw.clone();
        Callback::from(move |e: InputEvent| {
            let i: HtmlInputElement = e.target_unchecked_into();
            tags_raw.set(i.value());
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
        let tags_raw = tags_raw.clone();
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
                tags: text_to_tags(&tags_raw),
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
                    {"TAGS "}
                    <span style="color:var(--text-ghost)">{"(comma-separated)"}</span>
                </label>
                <input class="form-input" type="text" placeholder="triage, nightly"
                    value={(*tags_raw).clone()} oninput={on_tags} autocomplete="off" spellcheck="false" />
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

// ─── Bulk action bar ──────────────────────────────────────────────────────────

#[derive(Properties, PartialEq)]
pub struct RoutineBulkBarProps {
    pub count: usize,
    pub on_enable: Callback<()>,
    pub on_disable: Callback<()>,
    pub on_delete: Callback<()>,
    pub on_clear: Callback<()>,
}

/// Floating bulk-action toolbar. Hidden until at least one routine is selected.
///
/// Best-practice (Eleken UX guide, GitHub Actions): the bar appears in-context
/// as soon as a row is selected, shows the count, and offers primary actions
/// (enable/disable/delete) plus a clear affordance — no separate "actions" menu
/// needed.
#[function_component(RoutineBulkBar)]
pub fn routine_bulk_bar(props: &RoutineBulkBarProps) -> Html {
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
pub struct RoutineBulkDeleteProps {
    pub count: usize,
    pub on_cancel: Callback<()>,
    pub on_confirm: Callback<()>,
}

#[function_component(RoutineBulkDeleteDialog)]
pub fn routine_bulk_delete_dialog(props: &RoutineBulkDeleteProps) -> Html {
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
                <div class="confirm-title">{"⚠ DELETE ROUTINES"}</div>
                <div class="confirm-msg">
                    { format!("Delete {} selected routine(s)? This cannot be undone.", props.count) }
                </div>
                <div class="confirm-acts">
                    <button class="btn btn-ghost btn-sm" onclick={on_cancel}>{"CANCEL"}</button>
                    <button class="btn btn-danger btn-sm" onclick={on_confirm}>{"DELETE"}</button>
                </div>
            </div>
        </div>
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

    let load = {
        let id = props.id.clone();
        let flags = flags.clone();
        let loading = loading.clone();
        let err = err.clone();
        move || {
            let id = id.clone();
            let flags = flags.clone();
            let loading = loading.clone();
            let err = err.clone();
            loading.set(true);
            spawn_local(async move {
                match api_flags(&id).await {
                    Ok(list) => {
                        flags.set(list);
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
        html! {
            <div class="flags-list">
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
                    html! {
                        <div class="flag-item" key={flag.filename.clone()}>
                            <div class="flag-item-hd">
                                <span class="flag-type">{&flag.flag_type}</span>
                                <span class="flag-scope">{scope_label}</span>
                                <button class="btn btn-ghost btn-sm" onclick={on_resolve}>{"RESOLVE"}</button>
                            </div>
                            <div class="flag-desc">{&flag.description}</div>
                        </div>
                    }
                }) }
            </div>
        }
    };

    html! {
        <main class="logs-page">
            <div class="page-hd">
                <button class="btn btn-ghost btn-sm" onclick={on_back}>{"← BACK"}</button>
                <div class="page-title">{format!("FLAGS / {}", props.title)}</div>
                <button class="btn-refresh" title="Refresh" aria-label="Refresh" onclick={on_refresh}>{"↻"}</button>
            </div>
            {body}
        </main>
    }
}

#[cfg(test)]
#[path = "routines_tests.rs"]
mod routines_tests;
