//! The Overview page's fleet-wide "recent runs" table. Split out of `overview.rs` to keep that
//! file under the line-count gate; used only by `OverviewPage`.
//!
//! A merged, fleet-wide "what just ran" table: the most recent runs across every routine, each
//! showing which routine it belongs to, when it ran, and its outcome. Complements
//! `overview_upcoming`'s "UPCOMING RUNS" (future fires) with the equivalent view of the past. A
//! routine's own HISTORY page (Routines tab) shows its full per-routine run list; this is only the
//! fleet-wide recent slice.

use yew::prelude::*;
use yew_router::prelude::*;

use crate::reltime;
use crate::routines::{run_status_class, run_status_label, FleetRunSummary, RoutineHistoryQuery};
use crate::Route;

#[derive(Properties, PartialEq)]
pub(crate) struct RecentRunsTableProps {
    pub(crate) runs: Vec<FleetRunSummary>,
    pub(crate) loading: bool,
}

#[function_component(RecentRunsTable)]
pub(crate) fn recent_runs_table(props: &RecentRunsTableProps) -> Html {
    if props.loading {
        return html! { <div class="table-wrap"><div class="empty"><div class="spinner"></div></div></div> };
    }
    if props.runs.is_empty() {
        return html! {
            <div class="table-wrap">
                <div class="empty">
                    <div class="empty-icon">{"⧗"}</div>
                    <div class="empty-msg">{"NO RUNS YET"}</div>
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
                        <th>{"STARTED"}</th>
                        <th>{"STATUS"}</th>
                        <th>{"EXIT CODE"}</th>
                    </tr>
                </thead>
                <tbody>
                    { for props.runs.iter().map(|run| html! {
                        <tr key={run.workbench.clone()}>
                            <td>
                                <Link<Route, RoutineHistoryQuery>
                                    to={Route::Routines}
                                    query={Some(RoutineHistoryQuery { history: run.routine_id.clone() })}
                                >{ &run.routine_title }</Link<Route, RoutineHistoryQuery>>
                            </td>
                            <td><div class="cell-time">{reltime(run.started_at)}</div></td>
                            <td><span class={run_status_class(run.status)}>{run_status_label(run.status)}</span></td>
                            <td>{ run.exit_code.map(|c| c.to_string()).unwrap_or_else(|| "—".to_string()) }</td>
                        </tr>
                    }) }
                </tbody>
            </table>
        </div>
    }
}
