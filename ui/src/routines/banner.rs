//! Global lock banner, KPI stats bar, view toggle, and group-by selector shown
//! above the routines list.

use chrono::{DateTime, Duration, Local};
use web_sys::HtmlSelectElement;
use yew::prelude::*;

use crate::schedule::fires_within;

use super::filter::{is_routine_snoozed, RoutineStatusFacet, DUE_SOON_WINDOW_SECS};
use super::model::{LockStatus, Routine};
use super::state::{RGroupBy, RView};

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
        .filter(|r| {
            r.enabled
                && !is_routine_snoozed(r, props.now)
                && fires_within(&r.schedule, props.now, window)
        })
        .count();
    let snoozed = props
        .routines
        .iter()
        .filter(|r| r.enabled && is_routine_snoozed(r, props.now))
        .count();
    let dormant = props
        .routines
        .iter()
        .filter(|r| r.enabled && r.machines.is_empty())
        .count();
    let flags: usize = props.routines.iter().map(|r| r.flag_count).sum();
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
            { mk(RoutineStatusFacet::Dormant, "DORMANT", dormant, if dormant > 0 { "dormant has-dormant" } else { "dormant" }) }
            { mk(RoutineStatusFacet::DueSoon, "DUE SOON", due_soon, "due") }
            { mk(RoutineStatusFacet::Snoozed, "SNOOZED", snoozed, "snoozed") }
            { mk(RoutineStatusFacet::HasFlags, "FLAGS", flags, if flags > 0 { "flags has-flags" } else { "flags" }) }
            { mk(RoutineStatusFacet::AgentUnregistered, "UNREGISTERED AGENT", unreg, if unreg > 0 { "unreg has-unreg" } else { "unreg" }) }
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
                { for [RGroupBy::None, RGroupBy::Agent, RGroupBy::Machine, RGroupBy::Status, RGroupBy::Health].iter()
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
