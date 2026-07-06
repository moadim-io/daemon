//! The Overview page's KPI tile row: scheduled/enabled/due-soon/attention/
//! disabled/dormant/flags/snoozed counts plus the next-run countdown. Split
//! out of `overview.rs` to keep that file under the line-count gate; used only
//! by `OverviewPage`.

use yew::prelude::*;

use crate::overview::Kpis;

#[derive(Properties, PartialEq)]
pub(crate) struct OverviewStatsProps {
    pub(crate) kpis: Kpis,
    pub(crate) next_run: Option<String>,
}

#[function_component(OverviewStats)]
pub(crate) fn overview_stats(props: &OverviewStatsProps) -> Html {
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
