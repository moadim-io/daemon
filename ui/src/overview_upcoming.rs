//! The Overview page's "upcoming runs" table. Split out of `overview.rs` to keep
//! that file under the line-count gate; used only by `OverviewPage`.

use chrono::{DateTime, Local};
use yew::prelude::*;
use yew_router::prelude::*;

use crate::overview::{Kind, UpcomingRun};
use crate::schedule::{fmt_until, fmt_when};
use crate::Route;

#[derive(Properties, PartialEq)]
pub(crate) struct UpcomingTableProps {
    pub(crate) runs: Vec<UpcomingRun>,
    pub(crate) now: DateTime<Local>,
    pub(crate) loading: bool,
    pub(crate) error: Option<String>,
    pub(crate) on_trigger: Callback<(Kind, String)>,
}

#[function_component(UpcomingTable)]
pub(crate) fn upcoming_table(props: &UpcomingTableProps) -> Html {
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
