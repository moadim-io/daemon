//! The HEATMAP page: a forward-looking 7-day × 24-hour fire-density grid that
//! aggregates the next week's schedule of every enabled routine into one
//! color-coded matrix, so an operator can see fleet-wide busy windows,
//! scheduling collisions, and open slots at a glance.
//!
//! Best practice (cron/job-scheduler operations — cronheatmap.com, Cronitor,
//! Airflow's schedule-heatmap, and observability tools like Grafana/Datadog):
//! a 2-D hour-of-day × day heatmap surfaces activity-density patterns that a
//! flat job list hides. The dashboard already has a single-DAY timeline
//! (`day_timeline`) and per-page counts, but no multi-day, fleet-wide density
//! view; this closes that gap.
//!
//! The aggregation is pure and host-tested (see `schedule_heatmap_tests.rs`):
//! the grid math takes a list of schedule sources plus a fixed `now` and is free
//! of any DOM/wasm dependency. The component is a thin shell that fetches the
//! existing `/api/v1/routines` records, maps them to sources, and renders the
//! computed grid — no backend or API change.

#[cfg(test)]
use chrono::DateTime;
use chrono::{Local, NaiveDate, Timelike};
use gloo_timers::future::TimeoutFuture;
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;

use crate::overview::fetch_routines;
#[cfg(test)]
use crate::overview::Kind;
use crate::refresh::{RefreshControl, RefreshInterval};
use crate::routines::Routine;
#[cfg(test)]
use crate::schedule_heatmap_grid::HeatSource;
use crate::schedule_heatmap_grid::{
    compute_heatmap, day_label, day_totals, hour_totals, intensity_level, peak_label, sources_of,
    HeatFilter, Heatmap, HEAT_DAYS, HEAT_HOURS,
};

/// How often the live "now" advances so the grid (and its today/current-hour
/// highlight) rolls forward between fetches.
const TICK_MS: u32 = 60_000;

/// Loaded state for the heatmap shell.
#[derive(Clone, PartialEq, Default)]
struct Data {
    routines: Vec<Routine>,
    loading: bool,
    error: Option<String>,
}

#[function_component(HeatmapPage)]
pub fn heatmap_page() -> Html {
    let data = use_state(|| Data {
        loading: true,
        ..Data::default()
    });
    let now = use_state(Local::now);
    let filter = use_state(|| HeatFilter::All);
    let interval = use_state(crate::refresh::load_interval);
    let updated_at = use_state(|| 0.0_f64);

    // Fetch the routine record list.
    let load = {
        let data = data.clone();
        let updated_at = updated_at.clone();
        move || {
            let data = data.clone();
            let updated_at = updated_at.clone();
            spawn_local(async move {
                let routines = fetch_routines().await;
                let error = routines.as_ref().err().cloned();
                updated_at.set(js_sys::Date::now());
                data.set(Data {
                    routines: routines.unwrap_or_default(),
                    loading: false,
                    error,
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

    // Advance "now" so the grid rolls forward between fetches.
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

    let set_filter = {
        let filter = filter.clone();
        Callback::from(move |next: HeatFilter| filter.set(next))
    };

    let now_val = *now;
    let today = now_val.date_naive();
    let current_hour = now_val.hour() as usize;
    let sources = sources_of(&data.routines);
    let map = compute_heatmap(&sources, now_val, *filter);

    html! {
        <main>
            <div class="section-hd">
                <span class="section-label">{"SCHEDULE HEATMAP"}</span>
                <FilterTabs active={*filter} on_pick={set_filter} />
                <RefreshControl
                    interval={*interval}
                    updated_at_ms={*updated_at}
                    on_change={on_set_interval}
                />
            </div>
            <HeatStats map={map.clone()} today={today} />
            <HeatGrid
                map={map}
                today={today}
                current_hour={current_hour}
                loading={data.loading}
                error={data.error.clone()}
            />
        </main>
    }
}

#[derive(Properties, PartialEq)]
struct FilterTabsProps {
    active: HeatFilter,
    on_pick: Callback<HeatFilter>,
}

/// The All / Routines source filter, mirroring the dashboard's tab look.
#[function_component(FilterTabs)]
fn filter_tabs(props: &FilterTabsProps) -> Html {
    let render = |kind: HeatFilter| {
        let on_pick = props.on_pick.clone();
        let onclick = Callback::from(move |_: MouseEvent| on_pick.emit(kind));
        let cls = if props.active == kind {
            "hm-filter-btn active"
        } else {
            "hm-filter-btn"
        };
        html! {
            <button class={cls} aria-pressed={(props.active == kind).to_string()} {onclick}>
                {kind.label()}
            </button>
        }
    };
    html! {
        <div class="hm-filter" role="group" aria-label="Source filter">
            { render(HeatFilter::All) }
            { render(HeatFilter::Routine) }
        </div>
    }
}

#[derive(Properties, PartialEq)]
struct HeatStatsProps {
    map: Heatmap,
    today: NaiveDate,
}

/// The KPI row: total fires this week, the busiest window callout, and how many
/// of the 168 hour-slots stay open.
#[function_component(HeatStats)]
fn heat_stats(props: &HeatStatsProps) -> Html {
    let map = &props.map;
    let busiest = peak_label(map, props.today).unwrap_or_else(|| "—".into());
    let open_slots = (HEAT_DAYS * HEAT_HOURS) as u32 - filled_cells(map);
    html! {
        <div class="stats">
            <div class="stat-card all">
                <div class="stat-label">{"FIRES / 7 DAYS"}</div>
                <div class="stat-val">{map.total}</div>
            </div>
            <div class="stat-card due">
                <div class="stat-label">{"BUSIEST WINDOW"}</div>
                <div class="stat-val stat-val-sm c-red">{busiest}</div>
            </div>
            <div class="stat-card enabled">
                <div class="stat-label">{"PEAK / HOUR"}</div>
                <div class="stat-val c-accent">{map.max_cell}</div>
            </div>
            <div class="stat-card disabled">
                <div class="stat-label">{"SOURCES"}</div>
                <div class="stat-val">{map.sources}</div>
            </div>
            <div class="stat-card system">
                <div class="stat-label">{"OPEN SLOTS"}</div>
                <div class="stat-val stat-val-sm">{format!("{open_slots} / {}", HEAT_DAYS * HEAT_HOURS)}</div>
            </div>
        </div>
    }
}

/// How many of the grid's cells hold at least one fire.
fn filled_cells(map: &Heatmap) -> u32 {
    map.grid
        .iter()
        .flat_map(|hours| hours.iter())
        .filter(|&&count| count > 0)
        .count() as u32
}

#[derive(Properties, PartialEq)]
struct HeatGridProps {
    map: Heatmap,
    today: NaiveDate,
    current_hour: usize,
    loading: bool,
    error: Option<String>,
}

#[function_component(HeatGrid)]
fn heat_grid(props: &HeatGridProps) -> Html {
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
    if props.map.total == 0 {
        return html! {
            <div class="table-wrap">
                <div class="empty">
                    <div class="empty-icon">{"▦"}</div>
                    <div class="empty-msg">{"NOTHING SCHEDULED"}</div>
                    <div class="empty-sub">{"no enabled routine fires in the next 7 days"}</div>
                </div>
            </div>
        };
    }

    let map = &props.map;
    let max = map.max_cell;
    let day_sums = day_totals(map);
    let hour_sums = hour_totals(map);

    html! {
        <>
        <div class="hm-wrap">
            <table class="heatmap">
                <thead>
                    <tr>
                        <th class="hm-corner" scope="col">{"DAY \\ HR"}</th>
                        { for (0..HEAT_HOURS).map(|hour| {
                            let cls = if hour == props.current_hour { "hm-hcol now" } else { "hm-hcol" };
                            html! { <th class={cls} scope="col">{format!("{hour:02}")}</th> }
                        }) }
                        <th class="hm-rowtot" scope="col">{"Σ"}</th>
                    </tr>
                </thead>
                <tbody>
                    { for (0..HEAT_DAYS).map(|day| {
                        let row_cls = if day == 0 { "hm-row today" } else { "hm-row" };
                        html! {
                            <tr class={row_cls}>
                                <th class="hm-daylabel" scope="row">{day_label(props.today, day)}</th>
                                { for (0..HEAT_HOURS).map(|hour| {
                                    let count = map.grid[day][hour];
                                    let level = intensity_level(count, max);
                                    let is_peak = props.map.peak == Some((day, hour));
                                    let mut cls = format!("hm-cell lvl-{level}");
                                    if is_peak { cls.push_str(" peak"); }
                                    if day == 0 && hour == props.current_hour { cls.push_str(" now"); }
                                    let title = format!(
                                        "{} {hour:02}:00 — {count} run{}",
                                        day_label(props.today, day),
                                        if count == 1 { "" } else { "s" }
                                    );
                                    html! {
                                        <td class={cls} title={title}>
                                            { if count > 0 { html! { <span class="hm-n">{count}</span> } } else { html!{} } }
                                        </td>
                                    }
                                }) }
                                <td class="hm-rowtot">{day_sums[day]}</td>
                            </tr>
                        }
                    }) }
                </tbody>
                <tfoot>
                    <tr>
                        <th class="hm-daylabel" scope="row">{"Σ"}</th>
                        { for (0..HEAT_HOURS).map(|hour| {
                            html! { <td class="hm-rowtot">{hour_sums[hour]}</td> }
                        }) }
                        <td class="hm-rowtot grand">{map.total}</td>
                    </tr>
                </tfoot>
            </table>
        </div>
        <HeatLegend />
        </>
    }
}

/// The intensity-ramp key shown under the grid.
#[function_component(HeatLegend)]
fn heat_legend() -> Html {
    html! {
        <div class="hm-legend">
            <span class="hm-legend-label">{"LESS"}</span>
            { for (0..=4u8).map(|level| html! {
                <span class={format!("hm-cell lvl-{level} hm-legend-swatch")}></span>
            }) }
            <span class="hm-legend-label">{"MORE"}</span>
            <span class="hm-legend-note">{"fires per weekday · hour across the next 7 days"}</span>
        </div>
    }
}

#[cfg(test)]
#[path = "schedule_heatmap_tests.rs"]
mod schedule_heatmap_tests;
