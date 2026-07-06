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

use crate::overview_attention::{attention_items, AttentionTable};
#[cfg(test)]
use crate::overview_attention::{attention_reason, AttentionReason};
use crate::overview_recent_runs::RecentRunsTable;
use crate::overview_stats::OverviewStats;
use crate::overview_upcoming::UpcomingTable;
use crate::refresh::{RefreshControl, RefreshInterval};
use crate::routines::{
    api_all_runs, api_unlock, FleetRunSummary, GlobalLockBanner, LockStatus, Routine,
};
use crate::schedule::{fires_within, fmt_until, next_fire_after};
use crate::ToastKind;

/// How many of the most recent runs across the fleet the overview panel shows.
pub(crate) const RECENT_RUNS_LIMIT: usize = 8;

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

/// Map a routine onto the shared schedule abstraction. Takes `now` explicitly
/// (rather than sampling the wall clock here) so this stays a pure, host-
/// testable function and stays in lockstep with the same `now` the rest of
/// the page's KPI/attention/upcoming-run math uses.
fn is_snoozed(routine: &Routine, now: DateTime<Local>) -> bool {
    routine
        .snoozed_until
        .is_some_and(|until| (until as i64) > now.timestamp())
        || routine.skip_runs.is_some_and(|n| n > 0)
}

fn from_routine(routine: &Routine, now: DateTime<Local>) -> SchedSource {
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
        snoozed: is_snoozed(routine, now),
    }
}

/// Map the routine record list into one `SchedSource` vector.
fn sources_of(routines: &[Routine], now: DateTime<Local>) -> Vec<SchedSource> {
    routines.iter().map(|r| from_routine(r, now)).collect()
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

async fn fetch_recent_runs() -> Vec<FleetRunSummary> {
    api_all_runs(RECENT_RUNS_LIMIT).await.unwrap_or_default()
}

/// Loaded state for the overview shell.
#[derive(Clone, PartialEq, Default)]
struct Data {
    routines: Vec<Routine>,
    loading: bool,
    error: Option<String>,
    lock_status: Option<LockStatus>,
    recent_runs: Vec<FleetRunSummary>,
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
                let recent_runs = fetch_recent_runs().await;
                updated_at.set(js_sys::Date::now());
                data.set(Data {
                    routines: routines.unwrap_or_default(),
                    loading: false,
                    error,
                    lock_status,
                    recent_runs,
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
    let sources = sources_of(&data.routines, now_val);
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
            <div class="section-hd">
                <span class="section-label">{"RECENT RUNS"}</span>
            </div>
            <RecentRunsTable runs={data.recent_runs.clone()} loading={data.loading} />
        </main>
    }
}

#[cfg(test)]
#[path = "overview_tests.rs"]
mod overview_tests;
