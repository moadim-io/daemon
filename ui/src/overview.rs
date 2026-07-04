//! The OVERVIEW landing page: a single-pane operations summary that aggregates
//! routines into at-a-glance KPI tiles and one merged "upcoming runs" schedule,
//! so an operator sees the whole system's near-future activity without opening
//! the routines tab.
//!
//! Best practice (operational dashboards / cron monitors like Cronitor, Temporal
//! and Cloud Scheduler): lead with a small set of color-coded KPI tiles, then a
//! single merged timeline of what fires next across every scheduled entity.
//!
//! The page reads the existing `/api/v1/routines` endpoint — no backend change.
//! All KPI/merge math lives in pure, host-tested functions below (see
//! `overview_tests.rs`); the component is a thin shell that maps the fetched
//! records into `SchedSource`s and renders the result.

use chrono::{DateTime, Duration, Local};
use gloo_net::http::Request;
use gloo_timers::future::TimeoutFuture;
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;
use yew_router::prelude::*;

use crate::refresh::{RefreshControl, RefreshInterval};
use crate::routines::{api_unlock, GlobalLockBanner, LockStatus, Routine};
use crate::schedule::{fires_within, fmt_until, fmt_when, next_fire_after};
use crate::{Route, ToastKind};

/// "Due soon" / "soon" window: an enabled entity whose next fire lands within
/// this many seconds is operationally urgent. Mirrors the per-page cron stats.
pub(crate) const DUE_SOON_WINDOW_SECS: i64 = 3_600;

/// How many of the soonest upcoming runs the merged timeline shows.
pub(crate) const UPCOMING_LIMIT: usize = 8;

/// How often the live "now" advances so countdowns re-render between fetches.
const TICK_MS: u32 = 10_000;

/// Which kind of scheduled entity a row/tile refers to.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Kind {
    /// A routine (`/routines`).
    Routine,
}

/// A schedule-bearing entity reduced to just what the overview math needs.
/// `Routine` maps onto this so the aggregation logic stays agnostic of its
/// full shape (and host-testable without wasm types).
#[derive(Clone, PartialEq, Debug)]
pub(crate) struct SchedSource {
    /// Always `Kind::Routine` for now; kept so the table/badge rendering and
    /// the shared `schedule_heatmap` aggregation stay kind-aware.
    pub kind: Kind,
    /// API id used to trigger the entity (routine UUID).
    pub id: String,
    /// Display name: the routine title.
    pub label: String,
    /// Raw cron expression used to compute the next fire.
    pub schedule: String,
    /// Server-provided human description of the schedule, when present.
    pub human: Option<String>,
    /// Whether the entity is currently enabled (disabled ones never fire).
    pub enabled: bool,
    /// `true` when the entity targets no machine (empty or all-blank list), so
    /// it is scheduled but fires nowhere. Drives the "dormant" triage rule.
    pub machines_empty: bool,
    /// Whether the routine's agent is registered. `Some(false)` when the
    /// routine's agent is missing.
    pub agent_registered: Option<bool>,
    /// Number of open flags raised against this entity.
    pub flag_count: usize,
    /// Whether scheduled fires are currently suppressed (snoozed or skip-runs active).
    pub snoozed: bool,
}

/// Aggregate counts shown as the KPI tile row.
#[derive(Clone, PartialEq, Debug, Default)]
pub(crate) struct Kpis {
    /// All scheduled routines.
    pub total: usize,
    /// Enabled entities.
    pub enabled: usize,
    /// Disabled entities.
    pub disabled: usize,
    /// Enabled entities firing within [`DUE_SOON_WINDOW_SECS`].
    pub due_soon: usize,
    /// Enabled-but-misconfigured entities (see [`attention_items`]).
    pub attention: usize,
    /// Total open flags across all entities.
    pub flags: usize,
    /// Enabled entities whose scheduled fires are currently suppressed.
    pub snoozed: usize,
    /// Enabled entities assigned to no machine (fires nowhere).
    pub dormant: usize,
}

/// Why an enabled entity needs attention. Listed in triage priority order: a
/// dormant entity outranks a dead schedule, which outranks a missing agent,
/// which outranks open flags, so each entity surfaces its single most
/// fundamental fault (see [`attention_reason`]).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum AttentionReason {
    /// Enabled but assigned to no machine — it fires nowhere.
    Dormant,
    /// Targets a machine, but the schedule yields no future fire (empty,
    /// invalid, or a one-shot already in the past) — it never runs again.
    DeadSchedule,
    /// A routine whose agent is not registered — every run errors out.
    AgentUnregistered,
    /// Agent raised one or more flags during a run — needs human review.
    HasOpenFlags,
}

impl AttentionReason {
    /// Triage priority; lower sorts first.
    pub(crate) fn rank(self) -> u8 {
        match self {
            AttentionReason::Dormant => 0,
            AttentionReason::DeadSchedule => 1,
            AttentionReason::AgentUnregistered => 2,
            AttentionReason::HasOpenFlags => 3,
        }
    }

    /// Short uppercase badge label for the ISSUE column.
    pub(crate) fn badge(self) -> &'static str {
        match self {
            AttentionReason::Dormant => "DORMANT",
            AttentionReason::DeadSchedule => "DEAD SCHEDULE",
            AttentionReason::AgentUnregistered => "AGENT MISSING",
            AttentionReason::HasOpenFlags => "OPEN FLAGS",
        }
    }

    /// Human explanation of the operational consequence.
    pub(crate) fn detail(self) -> &'static str {
        match self {
            AttentionReason::Dormant => "assigned to no machine — fires nowhere",
            AttentionReason::DeadSchedule => "schedule has no future fire — never runs again",
            AttentionReason::AgentUnregistered => "agent not registered — every run errors",
            AttentionReason::HasOpenFlags => "agent raised flags during a run — needs review",
        }
    }
}

/// One enabled-but-misconfigured entity surfaced in the NEEDS ATTENTION panel.
#[derive(Clone, PartialEq, Debug)]
pub(crate) struct AttentionItem {
    /// Always `Kind::Routine` for now.
    pub kind: Kind,
    /// Display name.
    pub label: String,
    /// The single most fundamental fault to fix.
    pub reason: AttentionReason,
    /// Open flag count; non-zero only when `reason == HasOpenFlags`.
    pub flag_count: usize,
}

/// One entry in the merged upcoming-runs timeline.
#[derive(Clone, PartialEq, Debug)]
pub(crate) struct UpcomingRun {
    /// Always `Kind::Routine` for now.
    pub kind: Kind,
    /// API id passed to the trigger endpoint.
    pub id: String,
    /// Display name.
    pub label: String,
    /// Human schedule description, when present.
    pub human: Option<String>,
    /// Raw cron expression, used as fallback when `human` is absent.
    pub schedule: String,
    /// The next fire instant.
    pub at: DateTime<Local>,
    /// Whether `at` lands within the due-soon window.
    pub soon: bool,
    /// Total open flags on this routine (0 when none).
    pub flag_count: usize,
}

/// Count the KPI tiles from `sources` as of `now`.
pub(crate) fn compute_kpis(sources: &[SchedSource], now: DateTime<Local>) -> Kpis {
    let total = sources.len();
    let enabled = sources.iter().filter(|s| s.enabled).count();
    let window = Duration::seconds(DUE_SOON_WINDOW_SECS);
    let due_soon = sources
        .iter()
        .filter(|s| s.enabled && !s.snoozed && fires_within(&s.schedule, now, window))
        .count();
    let flags = sources.iter().map(|s| s.flag_count).sum();
    let snoozed = sources.iter().filter(|s| s.enabled && s.snoozed).count();
    let dormant = sources
        .iter()
        .filter(|s| s.enabled && s.machines_empty)
        .count();
    Kpis {
        total,
        enabled,
        disabled: total - enabled,
        due_soon,
        attention: attention_items(sources, now).len(),
        flags,
        snoozed,
        dormant,
    }
}

/// The single most fundamental fault for an enabled `source`, or `None` when it
/// is healthy. Disabled entities are intentional and never flagged. Faults are
/// checked in priority order so each entity reports exactly one reason.
pub(crate) fn attention_reason(
    source: &SchedSource,
    now: DateTime<Local>,
) -> Option<AttentionReason> {
    if !source.enabled {
        return None;
    }
    if source.machines_empty {
        return Some(AttentionReason::Dormant);
    }
    if next_fire_after(&source.schedule, now).is_none() {
        return Some(AttentionReason::DeadSchedule);
    }
    if source.agent_registered == Some(false) {
        return Some(AttentionReason::AgentUnregistered);
    }
    if source.flag_count > 0 {
        return Some(AttentionReason::HasOpenFlags);
    }
    None
}

/// All enabled-but-misconfigured entities, worst fault first, ties broken by
/// label for a stable order.
pub(crate) fn attention_items(sources: &[SchedSource], now: DateTime<Local>) -> Vec<AttentionItem> {
    let mut items: Vec<AttentionItem> = sources
        .iter()
        .filter_map(|s| {
            attention_reason(s, now).map(|reason| AttentionItem {
                kind: s.kind,
                label: s.label.clone(),
                flag_count: s.flag_count,
                reason,
            })
        })
        .collect();
    items.sort_by(|a, b| {
        a.reason
            .rank()
            .cmp(&b.reason.rank())
            .then_with(|| a.label.cmp(&b.label))
    });
    items
}

/// The merged, soonest-first list of the next [`UPCOMING_LIMIT`] fires across
/// every enabled, non-snoozed source. Disabled, snoozed, and ones with no
/// valid future fire are dropped; ties on fire time break by label for a
/// stable order.
pub(crate) fn upcoming_runs(sources: &[SchedSource], now: DateTime<Local>) -> Vec<UpcomingRun> {
    let window = Duration::seconds(DUE_SOON_WINDOW_SECS);
    let mut runs: Vec<UpcomingRun> = sources
        .iter()
        .filter(|s| s.enabled && !s.snoozed)
        .filter_map(|s| {
            next_fire_after(&s.schedule, now).map(|at| UpcomingRun {
                kind: s.kind,
                id: s.id.clone(),
                label: s.label.clone(),
                human: s.human.clone(),
                schedule: s.schedule.clone(),
                at,
                soon: at - now <= window,
                flag_count: s.flag_count,
            })
        })
        .collect();
    runs.sort_by(|a, b| a.at.cmp(&b.at).then_with(|| a.label.cmp(&b.label)));
    runs.truncate(UPCOMING_LIMIT);
    runs
}

/// Short relative countdown to the very next fire across all sources, e.g.
/// "in 4m", or `None` when nothing is scheduled to fire.
pub(crate) fn next_run_summary(runs: &[UpcomingRun], now: DateTime<Local>) -> Option<String> {
    runs.first().map(|r| fmt_until(now, r.at))
}

/// `true` when no entry names a real machine — an empty list, or one holding
/// only blank/whitespace entries (which the scheduler silently ignores).
fn targets_no_machine(machines: &[String]) -> bool {
    machines.iter().all(|m| m.trim().is_empty())
}

/// Map a routine onto the shared schedule abstraction.
fn is_snoozed(routine: &Routine) -> bool {
    let now_secs = (js_sys::Date::now() / 1000.0) as u64;
    routine.snoozed_until.is_some_and(|until| until > now_secs)
        || routine.skip_runs.is_some_and(|n| n > 0)
}

fn from_routine(routine: &Routine) -> SchedSource {
    SchedSource {
        kind: Kind::Routine,
        id: routine.id.clone(),
        label: routine.title.clone(),
        schedule: routine.schedule.clone(),
        human: routine.schedule_description.clone(),
        enabled: routine.enabled,
        machines_empty: targets_no_machine(&routine.machines),
        agent_registered: Some(routine.agent_registered),
        flag_count: routine.flag_count,
        snoozed: is_snoozed(routine),
    }
}

/// Map the routine record list into one `SchedSource` vector.
fn sources_of(routines: &[Routine]) -> Vec<SchedSource> {
    routines.iter().map(from_routine).collect()
}

async fn api_trigger_routine(id: &str) -> Result<(), String> {
    let resp = Request::post(&format!("/api/v1/routines/{id}/trigger"))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if resp.ok() {
        Ok(())
    } else {
        Err(format!("HTTP {}", resp.status()))
    }
}

pub(crate) async fn fetch_routines() -> Result<Vec<Routine>, String> {
    Request::get("/api/v1/routines")
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json::<Vec<Routine>>()
        .await
        .map_err(|e| e.to_string())
}

async fn fetch_lock_status() -> Option<LockStatus> {
    Request::get("/api/v1/routines/lock")
        .send()
        .await
        .ok()?
        .json::<LockStatus>()
        .await
        .ok()
}

/// Loaded state for the overview shell.
#[derive(Clone, PartialEq, Default)]
struct Data {
    routines: Vec<Routine>,
    loading: bool,
    error: Option<String>,
    lock_status: Option<LockStatus>,
}

#[derive(Properties, PartialEq)]
pub struct OverviewPageProps {
    pub on_toast: Callback<(String, ToastKind)>,
}

#[function_component(OverviewPage)]
pub fn overview_page(props: &OverviewPageProps) -> Html {
    let data = use_state(|| Data {
        loading: true,
        ..Data::default()
    });
    let now = use_state(Local::now);
    let interval = use_state(crate::refresh::load_interval);
    let updated_at = use_state(|| 0.0_f64);

    // Fetch the routine record list and lock status together.
    let load = {
        let data = data.clone();
        let updated_at = updated_at.clone();
        move || {
            let data = data.clone();
            let updated_at = updated_at.clone();
            spawn_local(async move {
                let routines = fetch_routines().await;
                let error = routines.as_ref().err().cloned();
                let lock_status = fetch_lock_status().await;
                updated_at.set(js_sys::Date::now());
                data.set(Data {
                    routines: routines.unwrap_or_default(),
                    loading: false,
                    error,
                    lock_status,
                });
            });
        }
    };

    // Load on mount.
    {
        let load = load.clone();
        use_effect_with((), move |_| load());
    }

    // Auto-refresh loop, re-armed when the interval changes.
    {
        use std::cell::Cell;
        use std::rc::Rc;
        let load = load.clone();
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
                        load();
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

    // Advance "now" so countdowns re-render between fetches.
    {
        let now = now.clone();
        use_effect_with((), move |_| {
            spawn_local(async move {
                loop {
                    TimeoutFuture::new(TICK_MS).await;
                    now.set(Local::now());
                }
            });
        });
    }

    let on_trigger = {
        let on_toast = props.on_toast.clone();
        Callback::from(move |(kind, id): (Kind, String)| {
            let on_toast = on_toast.clone();
            spawn_local(async move {
                let result = match kind {
                    Kind::Routine => api_trigger_routine(&id).await,
                };
                match result {
                    Ok(()) => on_toast.emit(("Triggered".into(), ToastKind::Ok)),
                    Err(e) => on_toast.emit((format!("Trigger failed: {e}"), ToastKind::Err)),
                }
            });
        })
    };

    let now_val = *now;
    let sources = sources_of(&data.routines);
    let kpis = compute_kpis(&sources, now_val);
    let attention = attention_items(&sources, now_val);
    let runs = upcoming_runs(&sources, now_val);
    let next_run = next_run_summary(&runs, now_val);

    let lock_status_for_banner = data.lock_status.clone();
    let on_unlock = {
        let data = data.clone();
        Callback::from(move |_: MouseEvent| {
            let data = data.clone();
            spawn_local(async move {
                if let Ok(status) = api_unlock("all").await {
                    let mut next = (*data).clone();
                    next.lock_status = Some(status);
                    data.set(next);
                }
            });
        })
    };

    html! {
        <main>
            <GlobalLockBanner status={lock_status_for_banner} on_unlock={on_unlock} />
            <OverviewStats kpis={kpis} next_run={next_run} />
            {
                // Only render the triage panel when something is actually broken,
                // so a healthy fleet stays uncluttered.
                if !attention.is_empty() {
                    html! {
                        <>
                            <div class="section-hd">
                                <span class="section-label attn">{"NEEDS ATTENTION"}</span>
                            </div>
                            <AttentionTable items={attention} />
                        </>
                    }
                } else {
                    html! {}
                }
            }
            <div class="section-hd">
                <span class="section-label">{"UPCOMING RUNS"}</span>
                <div class="section-acts">
                    <RefreshControl
                        interval={*interval}
                        updated_at_ms={*updated_at}
                        on_change={on_set_interval}
                    />
                </div>
            </div>
            <UpcomingTable
                runs={runs}
                now={now_val}
                loading={data.loading}
                error={data.error.clone()}
                on_trigger={on_trigger}
            />
        </main>
    }
}

#[derive(Properties, PartialEq)]
struct OverviewStatsProps {
    kpis: Kpis,
    next_run: Option<String>,
}

#[function_component(OverviewStats)]
fn overview_stats(props: &OverviewStatsProps) -> Html {
    let k = &props.kpis;
    let next = props.next_run.clone().unwrap_or_else(|| "—".into());
    html! {
        <div class="stats">
            <div class="stat-card all">
                <div class="stat-label">{"SCHEDULED"}</div>
                <div class="stat-val">{k.total}</div>
            </div>
            <div class="stat-card enabled">
                <div class="stat-label">{"ENABLED"}</div>
                <div class="stat-val c-accent">{k.enabled}</div>
            </div>
            <div class="stat-card due">
                <div class="stat-label">{"DUE SOON"}</div>
                <div class="stat-val c-red">{k.due_soon}</div>
            </div>
            <div class="stat-card attention">
                <div class="stat-label">{"ATTENTION"}</div>
                <div class={classes!("stat-val", if k.attention > 0 { "c-red" } else { "c-accent" })}>
                    {k.attention}
                </div>
            </div>
            <div class="stat-card disabled">
                <div class="stat-label">{"DISABLED"}</div>
                <div class="stat-val c-amber">{k.disabled}</div>
            </div>
            <div class={classes!("stat-card", if k.dormant > 0 { "has-dormant" } else { "dormant" })}>
                <div class="stat-label">{"DORMANT"}</div>
                <div class={classes!("stat-val", if k.dormant > 0 { "c-amber" } else { "" })}>
                    {k.dormant}
                </div>
            </div>
            <div class="stat-card flags">
                <div class="stat-label">{"FLAGS"}</div>
                <div class={classes!("stat-val", if k.flags > 0 { "c-red" } else { "c-accent" })}>
                    {k.flags}
                </div>
            </div>
            <div class="stat-card snoozed">
                <div class="stat-label">{"SNOOZED"}</div>
                <div class={classes!("stat-val", if k.snoozed > 0 { "c-amber" } else { "c-accent" })}>
                    {k.snoozed}
                </div>
            </div>
            <div class="stat-card system">
                <div class="stat-label">{"NEXT RUN"}</div>
                <div class="stat-val stat-val-sm">{next}</div>
            </div>
        </div>
    }
}

#[derive(Properties, PartialEq)]
struct AttentionTableProps {
    items: Vec<AttentionItem>,
}

/// The NEEDS ATTENTION triage table: one row per enabled-but-broken entity,
/// worst fault first. Rendered only when `items` is non-empty (see the page),
/// so this component never has to handle the loading/empty states.
#[function_component(AttentionTable)]
fn attention_table(props: &AttentionTableProps) -> Html {
    html! {
        <div class="table-wrap attn-wrap">
            <table>
                <thead>
                    <tr>
                        <th>{"TYPE"}</th>
                        <th>{"NAME"}</th>
                        <th>{"ISSUE"}</th>
                        <th>{"DETAIL"}</th>
                    </tr>
                </thead>
                <tbody>
                    { for props.items.iter().enumerate().map(|(i, item)| {
                        let (badge, badge_cls, to) = match item.kind {
                            Kind::Routine => ("ROUTINE", "kind-badge routine", Route::Routines),
                        };
                        html! {
                            <tr key={i.to_string()}>
                                <td><span class={badge_cls}>{badge}</span></td>
                                <td>
                                    <Link<Route> classes={classes!("ov-name-link")} to={to}>
                                        {item.label.clone()}
                                    </Link<Route>>
                                </td>
                                <td><span class="attn-badge">{item.reason.badge()}</span></td>
                                <td class="attn-detail">{
                                    if item.reason == AttentionReason::HasOpenFlags && item.flag_count > 0 {
                                        format!("{} open flag{} — needs review", item.flag_count, if item.flag_count == 1 { "" } else { "s" })
                                    } else {
                                        item.reason.detail().to_string()
                                    }
                                }</td>
                            </tr>
                        }
                    }) }
                </tbody>
            </table>
        </div>
    }
}

#[derive(Properties, PartialEq)]
struct UpcomingTableProps {
    runs: Vec<UpcomingRun>,
    now: DateTime<Local>,
    loading: bool,
    error: Option<String>,
    on_trigger: Callback<(Kind, String)>,
}

#[function_component(UpcomingTable)]
fn upcoming_table(props: &UpcomingTableProps) -> Html {
    if let Some(err) = &props.error {
        return html! {
            <div class="table-wrap">
                <div class="empty">
                    <div class="empty-icon">{"⚠"}</div>
                    <div class="empty-msg">{"FAILED TO LOAD"}</div>
                    <div class="empty-sub">{err.clone()}</div>
                </div>
            </div>
        };
    }
    if props.loading {
        return html! {
            <div class="table-wrap">
                <div class="empty"><div class="spinner"></div></div>
            </div>
        };
    }
    if props.runs.is_empty() {
        return html! {
            <div class="table-wrap">
                <div class="empty">
                    <div class="empty-icon">{"◷"}</div>
                    <div class="empty-msg">{"NO UPCOMING RUNS"}</div>
                    <div class="empty-sub">{"no enabled routine is scheduled to fire"}</div>
                </div>
            </div>
        };
    }

    let now = props.now;
    html! {
        <div class="table-wrap">
            <table>
                <thead>
                    <tr>
                        <th>{"TYPE"}</th>
                        <th>{"NAME"}</th>
                        <th>{"SCHEDULE"}</th>
                        <th>{"NEXT RUN"}</th>
                        <th></th>
                    </tr>
                </thead>
                <tbody>
                    { for props.runs.iter().enumerate().map(|(i, run)| {
                        let (badge, badge_cls, to) = match run.kind {
                            Kind::Routine => ("ROUTINE", "kind-badge routine", Route::Routines),
                        };
                        let until_cls = if run.soon { "cell-next-until soon" } else { "cell-next-until" };
                        let kind = run.kind;
                        let id = run.id.clone();
                        let on_trigger = props.on_trigger.clone();
                        let onclick = Callback::from(move |_: MouseEvent| {
                            on_trigger.emit((kind, id.clone()));
                        });
                        html! {
                            <tr key={i.to_string()}>
                                <td><span class={badge_cls}>{badge}</span></td>
                                <td>
                                    <Link<Route> classes={classes!("ov-name-link")} to={to}>
                                        {run.label.clone()}
                                    </Link<Route>>
                                    if run.flag_count > 0 {
                                        <span class="ov-flag-badge" title={format!("{} open flag{}", run.flag_count, if run.flag_count == 1 { "" } else { "s" })}>
                                            {format!("⚑ {}", run.flag_count)}
                                        </span>
                                    }
                                </td>
                                <td>
                                    <div class="cell-schedule-human">
                                        {run.human.clone().unwrap_or_else(|| run.schedule.clone())}
                                    </div>
                                </td>
                                <td class="cell-next">
                                    <div class="cell-next-when">{fmt_when(now, run.at)}</div>
                                    <div class={until_cls}>{fmt_until(now, run.at)}</div>
                                </td>
                                <td class="cell-act">
                                    <button
                                        class="btn btn-sm btn-ghost run-now-btn"
                                        title="Trigger now"
                                        {onclick}
                                    >
                                        {"▶ RUN"}
                                    </button>
                                </td>
                            </tr>
                        }
                    }) }
                </tbody>
            </table>
        </div>
    }
}

#[cfg(test)]
#[path = "overview_tests.rs"]
mod overview_tests;
