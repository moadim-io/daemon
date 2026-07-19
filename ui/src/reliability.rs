//! The RELIABILITY page: ranks every routine by recent run outcomes so an operator can spot
//! what's actively broken or flaky without opening each routine's HISTORY tab individually.
//!
//! Best practice (CI/CD reliability dashboards — GitHub Actions Insights, `CircleCI` Insights,
//! Datadog Test Visibility): surface a success-rate ranking and flag flaky jobs (alternating
//! pass/fail) as their own category, distinct from steadily-failing ones — the response to each
//! differs (investigate vs. page on-call).
//!
//! The page reads the existing `GET /routines/runs` endpoint (already used by the Routines
//! table's inline sparkline column) — no backend change. All ranking/flakiness math lives in
//! pure, host-tested functions in `reliability_stats.rs`; this module is the thin data-fetching
//! shell that renders the result.

use gloo_timers::future::TimeoutFuture;
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;
use yew_router::prelude::*;

use crate::refresh::{load_interval, save_interval, RefreshControl, RefreshInterval};
use crate::reliability_stats::{
    compute_reliability, fleet_summary, rate_class, rate_label, streak_class, streak_label,
    FleetReliability, RoutineReliability,
};
use crate::routines::{api_all_runs, RoutineHistoryQuery};
use crate::Route;

/// Fleet-wide runs fetched to build the reliability sample. Mirrors the Routines table's
/// sparkline fetch cap (`GET /routines/runs` truncates its newest-first merged list to this
/// many total, across every routine) — high enough that an active fleet's routines each keep a
/// `SAMPLE_LEN`-sized window without an unbounded payload.
const FETCH_LIMIT: usize = 300;

/// Loaded state for the reliability page.
#[derive(Clone, PartialEq, Default)]
struct Data {
    items: Vec<RoutineReliability>,
    loading: bool,
}

#[function_component(ReliabilityPage)]
pub fn reliability_page() -> Html {
    let data = use_state(|| Data {
        loading: true,
        ..Data::default()
    });
    let interval = use_state(load_interval);
    let updated_at = use_state(|| 0.0_f64);

    let load = {
        let data = data.clone();
        let updated_at = updated_at.clone();
        move || {
            let data = data.clone();
            let updated_at = updated_at.clone();
            spawn_local(async move {
                let runs = api_all_runs(FETCH_LIMIT).await.unwrap_or_default();
                let items = compute_reliability(&runs);
                updated_at.set(js_sys::Date::now());
                data.set(Data {
                    items,
                    loading: false,
                });
            });
        }
    };

    {
        let load = load.clone();
        use_effect_with((), move |()| load());
    }

    {
        use std::cell::Cell;
        use std::rc::Rc;
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
            save_interval(next);
            interval.set(next);
        })
    };

    let summary = fleet_summary(&data.items);

    html! {
        <main>
            <div class="section-hd">
                <span class="section-label">{"RELIABILITY"}</span>
                <div class="section-acts">
                    <RefreshControl
                        interval={*interval}
                        updated_at_ms={*updated_at}
                        on_change={on_set_interval}
                    />
                </div>
            </div>
            <SummaryTiles summary={summary} />
            { render_table(&data.items, data.loading) }
        </main>
    }
}

#[derive(Properties, PartialEq)]
struct SummaryTilesProps {
    summary: FleetReliability,
}

/// The page's KPI tile row: fleet-wide success rate plus counts of routines currently failing
/// or flaky, reusing the same `.stats`/`.stat-card` layout as the Overview page.
#[function_component(SummaryTiles)]
fn summary_tiles(props: &SummaryTilesProps) -> Html {
    let s = &props.summary;
    html! {
        <div class="stats">
            <div class="stat-card all">
                <div class="stat-label">{"FLEET SUCCESS RATE"}</div>
                <div class="stat-val">{rate_label(s.success_rate())}</div>
            </div>
            <div class="stat-card due">
                <div class="stat-label">{"FAILING NOW"}</div>
                <div class={classes!("stat-val", if s.failing_count > 0 { "c-red" } else { "c-accent" })}>
                    {s.failing_count}
                </div>
            </div>
            <div class="stat-card disabled">
                <div class="stat-label">{"FLAKY"}</div>
                <div class={classes!("stat-val", if s.flaky_count > 0 { "c-amber" } else { "c-accent" })}>
                    {s.flaky_count}
                </div>
            </div>
            <div class="stat-card system">
                <div class="stat-label">{"SAMPLED RUNS"}</div>
                <div class="stat-val">{s.sample_size}</div>
            </div>
        </div>
    }
}

/// Renders the ranked reliability table (worst-first), or the loading/empty state.
fn render_table(items: &[RoutineReliability], loading: bool) -> Html {
    if loading {
        return html! { <div class="table-wrap"><div class="empty"><div class="spinner"></div></div></div> };
    }
    if items.is_empty() {
        return html! {
            <div class="table-wrap">
                <div class="empty">
                    <div class="empty-icon">{"⧗"}</div>
                    <div class="empty-msg">{"NO FINISHED RUNS YET"}</div>
                </div>
            </div>
        };
    }
    html! {
        <div class="table-wrap">
            <table>
                <thead>
                    <tr>
                        <th>{"ROUTINE"}</th>
                        <th>{"STREAK"}</th>
                        <th>{"SUCCESS RATE"}</th>
                        <th>{"SAMPLE"}</th>
                        <th>{"FLAKY"}</th>
                    </tr>
                </thead>
                <tbody>
                    { for items.iter().map(render_row) }
                </tbody>
            </table>
        </div>
    }
}

/// Renders one routine's reliability row.
fn render_row(item: &RoutineReliability) -> Html {
    let rate = item.success_rate();
    html! {
        <tr key={item.routine_id.clone()}>
            <td>
                <Link<Route, RoutineHistoryQuery>
                    to={Route::Routines}
                    query={Some(RoutineHistoryQuery { history: item.routine_id.clone() })}
                >{ &item.routine_title }</Link<Route, RoutineHistoryQuery>>
            </td>
            <td><span class={streak_class(item.streak)}>{streak_label(item.streak)}</span></td>
            <td><span class={rate_class(rate)}>{rate_label(rate)}</span></td>
            <td><span class="cell-meta">{item.sample_size}</span></td>
            <td>{ if item.is_flaky() { html! { <span class="run-status running">{"FLAKY"}</span> } } else { html! {} } }</td>
        </tr>
    }
}
