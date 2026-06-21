//! The OVERVIEW landing page: a single-pane operations summary that aggregates
//! both cron jobs and routines into at-a-glance KPI tiles and one merged
//! "upcoming runs" schedule, so an operator sees the whole system's near-future
//! activity without opening each tab.
//!
//! Best practice (operational dashboards / cron monitors like Cronitor, Temporal
//! and Cloud Scheduler): lead with a small set of color-coded KPI tiles, then a
//! single merged timeline of what fires next across every scheduled entity.
//!
//! The page reads the existing `/api/v1/cron-jobs` and `/api/v1/routines`
//! endpoints — no backend change. All KPI/merge math lives in pure, host-tested
//! functions below (see `overview_tests.rs`); the component is a thin shell that
//! maps the fetched records into `SchedSource`s and renders the result.

use chrono::{DateTime, Duration, Local};
use gloo_net::http::Request;
use gloo_timers::future::TimeoutFuture;
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;
use yew_router::prelude::*;

use crate::cron_jobs::CronJob;
use crate::routines::Routine;
use crate::schedule::{fires_within, fmt_until, fmt_when, next_fire_after};
use crate::Route;

/// "Due soon" / "soon" window: an enabled entity whose next fire lands within
/// this many seconds is operationally urgent. Mirrors the per-page cron stats.
pub(crate) const DUE_SOON_WINDOW_SECS: i64 = 3_600;

/// How many of the soonest upcoming runs the merged timeline shows.
pub(crate) const UPCOMING_LIMIT: usize = 8;

/// How often the page re-fetches the underlying records (counts can change as
/// jobs are toggled elsewhere).
const REFETCH_MS: u32 = 30_000;

/// How often the live "now" advances so countdowns re-render between fetches.
const TICK_MS: u32 = 10_000;

/// Which kind of scheduled entity a row/tile refers to.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Kind {
    /// A cron job (`/cron-jobs`).
    Cron,
    /// A routine (`/routines`).
    Routine,
}

/// A schedule-bearing entity reduced to just what the overview math needs.
/// Both `CronJob` and `Routine` map onto this so the aggregation logic stays
/// agnostic of their full shapes (and host-testable without wasm types).
#[derive(Clone, PartialEq, Debug)]
pub(crate) struct SchedSource {
    /// Cron job or routine.
    pub kind: Kind,
    /// Display name: the cron-job id or the routine title.
    pub label: String,
    /// Raw cron expression used to compute the next fire.
    pub schedule: String,
    /// Server-provided human description of the schedule, when present.
    pub human: Option<String>,
    /// Whether the entity is currently enabled (disabled ones never fire).
    pub enabled: bool,
}

/// Aggregate counts shown as the KPI tile row.
#[derive(Clone, PartialEq, Debug, Default)]
pub(crate) struct Kpis {
    /// All scheduled entities (cron jobs + routines).
    pub total: usize,
    /// Enabled entities.
    pub enabled: usize,
    /// Disabled entities.
    pub disabled: usize,
    /// Enabled entities firing within [`DUE_SOON_WINDOW_SECS`].
    pub due_soon: usize,
}

/// One entry in the merged upcoming-runs timeline.
#[derive(Clone, PartialEq, Debug)]
pub(crate) struct UpcomingRun {
    /// Cron job or routine.
    pub kind: Kind,
    /// Display name.
    pub label: String,
    /// Human schedule description, when present.
    pub human: Option<String>,
    /// The next fire instant.
    pub at: DateTime<Local>,
    /// Whether `at` lands within the due-soon window.
    pub soon: bool,
}

/// Count the KPI tiles from `sources` as of `now`.
pub(crate) fn compute_kpis(sources: &[SchedSource], now: DateTime<Local>) -> Kpis {
    let total = sources.len();
    let enabled = sources.iter().filter(|s| s.enabled).count();
    let window = Duration::seconds(DUE_SOON_WINDOW_SECS);
    let due_soon = sources
        .iter()
        .filter(|s| s.enabled && fires_within(&s.schedule, now, window))
        .count();
    Kpis {
        total,
        enabled,
        disabled: total - enabled,
        due_soon,
    }
}

/// The merged, soonest-first list of the next [`UPCOMING_LIMIT`] fires across
/// every enabled source. Disabled entities and ones with no valid future fire
/// are dropped; ties on fire time break by label for a stable order.
pub(crate) fn upcoming_runs(sources: &[SchedSource], now: DateTime<Local>) -> Vec<UpcomingRun> {
    let window = Duration::seconds(DUE_SOON_WINDOW_SECS);
    let mut runs: Vec<UpcomingRun> = sources
        .iter()
        .filter(|s| s.enabled)
        .filter_map(|s| {
            next_fire_after(&s.schedule, now).map(|at| UpcomingRun {
                kind: s.kind,
                label: s.label.clone(),
                human: s.human.clone(),
                at,
                soon: at - now <= window,
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

/// Map a cron job onto the shared schedule abstraction.
fn from_cron(job: &CronJob) -> SchedSource {
    SchedSource {
        kind: Kind::Cron,
        label: job.id.clone(),
        schedule: job.schedule.clone(),
        human: job.schedule_description.clone(),
        enabled: job.enabled,
    }
}

/// Map a routine onto the shared schedule abstraction.
fn from_routine(routine: &Routine) -> SchedSource {
    SchedSource {
        kind: Kind::Routine,
        label: routine.title.clone(),
        schedule: routine.schedule.clone(),
        human: routine.schedule_description.clone(),
        enabled: routine.enabled,
    }
}

/// Flatten both record lists into one `SchedSource` vector.
fn sources_of(crons: &[CronJob], routines: &[Routine]) -> Vec<SchedSource> {
    crons
        .iter()
        .map(from_cron)
        .chain(routines.iter().map(from_routine))
        .collect()
}

pub(crate) async fn fetch_crons() -> Result<Vec<CronJob>, String> {
    Request::get("/api/v1/cron-jobs")
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json::<Vec<CronJob>>()
        .await
        .map_err(|e| e.to_string())
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

/// Loaded state for the overview shell.
#[derive(Clone, PartialEq, Default)]
struct Data {
    crons: Vec<CronJob>,
    routines: Vec<Routine>,
    loading: bool,
    /// Set only when BOTH fetches fail, so a partial load still renders.
    error: Option<String>,
}

#[function_component(OverviewPage)]
pub fn overview_page() -> Html {
    let data = use_state(|| Data {
        loading: true,
        ..Data::default()
    });
    let now = use_state(Local::now);

    // Fetch both record lists; surface an error only when both fail.
    let load = {
        let data = data.clone();
        move || {
            let data = data.clone();
            spawn_local(async move {
                let (crons, routines) = futures_join(fetch_crons(), fetch_routines()).await;
                let error = match (&crons, &routines) {
                    (Err(ce), Err(re)) => Some(format!("{ce}; {re}")),
                    _ => None,
                };
                data.set(Data {
                    crons: crons.unwrap_or_default(),
                    routines: routines.unwrap_or_default(),
                    loading: false,
                    error,
                });
            });
        }
    };

    // Load on mount, then re-fetch on a slow cadence.
    {
        let load = load.clone();
        use_effect_with((), move |_| {
            load();
            spawn_local(async move {
                loop {
                    TimeoutFuture::new(REFETCH_MS).await;
                    load();
                }
            });
        });
    }

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

    let now_val = *now;
    let sources = sources_of(&data.crons, &data.routines);
    let kpis = compute_kpis(&sources, now_val);
    let runs = upcoming_runs(&sources, now_val);
    let next_run = next_run_summary(&runs, now_val);

    html! {
        <main>
            <OverviewStats kpis={kpis} next_run={next_run} />
            <div class="section-hd">
                <span class="section-label">{"UPCOMING RUNS"}</span>
            </div>
            <UpcomingTable
                runs={runs}
                now={now_val}
                loading={data.loading}
                error={data.error.clone()}
            />
        </main>
    }
}

/// Await two futures and return both results. A tiny local join so the page
/// needs no extra dependency; the two fetches are issued before the first await.
async fn futures_join<A, B>(
    a: impl std::future::Future<Output = A>,
    b: impl std::future::Future<Output = B>,
) -> (A, B) {
    (a.await, b.await)
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
            <div class="stat-card disabled">
                <div class="stat-label">{"DISABLED"}</div>
                <div class="stat-val c-amber">{k.disabled}</div>
            </div>
            <div class="stat-card system">
                <div class="stat-label">{"NEXT RUN"}</div>
                <div class="stat-val stat-val-sm">{next}</div>
            </div>
        </div>
    }
}

#[derive(Properties, PartialEq)]
struct UpcomingTableProps {
    runs: Vec<UpcomingRun>,
    now: DateTime<Local>,
    loading: bool,
    error: Option<String>,
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
                    <div class="empty-sub">{"no enabled job or routine is scheduled to fire"}</div>
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
                    </tr>
                </thead>
                <tbody>
                    { for props.runs.iter().enumerate().map(|(i, run)| {
                        let (badge, badge_cls, to) = match run.kind {
                            Kind::Cron => ("CRON", "kind-badge cron", Route::CronJobs),
                            Kind::Routine => ("ROUTINE", "kind-badge routine", Route::Routines),
                        };
                        let until_cls = if run.soon { "cell-next-until soon" } else { "cell-next-until" };
                        html! {
                            <tr key={i.to_string()}>
                                <td><span class={badge_cls}>{badge}</span></td>
                                <td>
                                    <Link<Route> classes={classes!("ov-name-link")} to={to}>
                                        {run.label.clone()}
                                    </Link<Route>>
                                </td>
                                <td>
                                    <div class="cell-schedule-human">
                                        {run.human.clone().unwrap_or_else(|| "—".into())}
                                    </div>
                                </td>
                                <td class="cell-next">
                                    <div class="cell-next-when">{fmt_when(now, run.at)}</div>
                                    <div class={until_cls}>{fmt_until(now, run.at)}</div>
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
