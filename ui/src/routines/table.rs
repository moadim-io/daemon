//! Routine table: sortable/groupable header, empty states, and the NEXT RUN cell
//! helper shared with the row component.

use std::collections::{BTreeSet, HashMap};

use chrono::{Duration, Local};
use yew::prelude::*;

use crate::schedule::{fmt_until, fmt_when, next_fire_after};

use super::filter::{is_routine_snoozed, snooze_detail, DUE_SOON_WINDOW_SECS};
use super::model::{FleetRunSummary, Routine};
use super::row::RoutineRow;
use super::state::{group_routines, RCol, RDir, RGroupBy};

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
    /// Each routine's recent runs (oldest to newest), keyed by routine id, backing the RUN
    /// HISTORY sparkline column.
    pub run_history: HashMap<String, Vec<FleetRunSummary>>,
    /// Fired when the user clicks a sortable column header.
    pub on_sort: Callback<RCol>,
    pub on_edit: Callback<String>,
    pub on_clone: Callback<String>,
    pub on_delete: Callback<(String, String)>,
    pub on_toggle: Callback<(String, bool)>,
    pub on_trigger: Callback<String>,
    pub on_logs: Callback<String>,
    pub on_history: Callback<String>,
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
                        <th>{"RUN HISTORY"}</th>
                        { sort_th("AGENT", RCol::Agent, props.sort_col, props.sort_dir, &props.on_sort) }
                        <th>{"REPOS"}</th>
                        <th>{"MACHINES"}</th>
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
                                        <td colspan="13">
                                            <span class="group-label">{label.clone()}</span>
                                            <span class="group-count">{format!("({count})")}</span>
                                        </td>
                                    </tr>
                                }
                                { for group.into_iter().map(|r| {
                                    let runs = props.run_history.get(&r.id).cloned().unwrap_or_default();
                                    html! {
                                    <RoutineRow
                                        key={r.id.clone()}
                                        routine={r.clone()}
                                        now={props.now}
                                        runs={runs}
                                        selected={props.selected.contains(&r.id)}
                                        on_select={props.on_select.clone()}
                                        on_edit={props.on_edit.clone()}
                                        on_clone={props.on_clone.clone()}
                                        on_delete={props.on_delete.clone()}
                                        on_toggle={props.on_toggle.clone()}
                                        on_trigger={props.on_trigger.clone()}
                                        on_logs={props.on_logs.clone()}
                                        on_history={props.on_history.clone()}
                                        on_flags={props.on_flags.clone()}
                                    />
                                    }
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
    if is_routine_snoozed(routine, now) {
        let detail = snooze_detail(routine, now);
        return html! {
            <div class="cell-next">
                <span class="cell-next muted">{"snoozed"}</span>
                if !detail.is_empty() {
                    <div class="cell-next-until muted">{detail}</div>
                }
            </div>
        };
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
