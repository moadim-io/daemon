//! A single routine table row: schedule preview, next/last fire, health badge,
//! and row actions.

use chrono::Local;
use yew::prelude::*;

use crate::reltime;
use crate::schedule::{fmt_until, fmt_when, next_fires};

use super::filter::{last_fire_at, routine_health};
use super::form::format_ttl;
use super::model::Routine;
use super::table::next_routine_run_cell;

#[derive(Properties, PartialEq)]
pub struct RowProps {
    pub routine: Routine,
    /// Reference instant for the NEXT RUN countdown.
    pub now: chrono::DateTime<Local>,
    /// Whether this row is currently selected.
    pub selected: bool,
    /// Fired when the selection checkbox is clicked.
    pub on_select: Callback<String>,
    pub on_edit: Callback<String>,
    pub on_clone: Callback<String>,
    pub on_delete: Callback<(String, String)>,
    pub on_toggle: Callback<(String, bool)>,
    pub on_trigger: Callback<String>,
    pub on_logs: Callback<String>,
    pub on_flags: Callback<String>,
}

#[function_component(RoutineRow)]
pub fn routine_row(props: &RowProps) -> Html {
    let preview_open = use_state(|| false);

    let r = &props.routine;
    let cron_text = r.schedule_description.as_deref().unwrap_or("—").to_string();
    let updated = reltime(r.updated_at);
    let repos = r.repositories.len();
    let machines: Vec<&str> = r
        .machines
        .iter()
        .map(|m| m.as_str())
        .filter(|m| !m.trim().is_empty())
        .collect();

    let on_edit = {
        let cb = props.on_edit.clone();
        let id = r.id.clone();
        Callback::from(move |_: MouseEvent| cb.emit(id.clone()))
    };
    let on_clone = {
        let cb = props.on_clone.clone();
        let id = r.id.clone();
        Callback::from(move |_: MouseEvent| cb.emit(id.clone()))
    };
    let on_delete = {
        let cb = props.on_delete.clone();
        let id = r.id.clone();
        let title = r.title.clone();
        Callback::from(move |_: MouseEvent| cb.emit((id.clone(), title.clone())))
    };
    let on_toggle = {
        let cb = props.on_toggle.clone();
        let id = r.id.clone();
        let enabled = r.enabled;
        Callback::from(move |_: Event| cb.emit((id.clone(), !enabled)))
    };
    let on_trigger = {
        let cb = props.on_trigger.clone();
        let id = r.id.clone();
        Callback::from(move |_: MouseEvent| cb.emit(id.clone()))
    };
    let on_logs = {
        let cb = props.on_logs.clone();
        let id = r.id.clone();
        Callback::from(move |_: MouseEvent| cb.emit(id.clone()))
    };
    let on_flags = {
        let cb = props.on_flags.clone();
        let id = r.id.clone();
        Callback::from(move |_: MouseEvent| cb.emit(id.clone()))
    };

    let on_select = {
        let cb = props.on_select.clone();
        let id = r.id.clone();
        Callback::from(move |_: MouseEvent| cb.emit(id.clone()))
    };

    let on_preview_toggle = {
        let preview_open = preview_open.clone();
        Callback::from(move |e: MouseEvent| {
            e.stop_propagation();
            preview_open.set(!*preview_open);
        })
    };

    let fires_panel = if *preview_open {
        let fires = next_fires(&r.schedule, props.now, 10);
        if fires.is_empty() {
            html! { <div class="fires-panel"><div class="fires-empty">{"— no future fires —"}</div></div> }
        } else {
            let now = props.now;
            html! {
                <div class="fires-panel">
                    <div class="fires-hd">{"NEXT 10 FIRES"}</div>
                    { for fires.iter().map(|t| html! {
                        <div class="fires-item">
                            <span class="fires-when">{fmt_when(now, *t)}</span>
                            <span class="fires-until">{fmt_until(now, *t)}</span>
                        </div>
                    }) }
                </div>
            }
        }
    } else {
        html! {}
    };

    let preview_btn_class = if *preview_open {
        "sched-preview-btn open"
    } else {
        "sched-preview-btn"
    };

    // LAST FIRE: most-recent trigger timestamp from last_fire_at, icon chosen by type.
    // ↻ = the most-recent fire was a manual trigger; ⏱ = it was a scheduled fire.
    let last_fire: Html = match last_fire_at(r) {
        None => html! { <span class="muted">{"—"}</span> },
        Some(ts) => {
            let manual = r.last_manual_trigger_at;
            let scheduled = r.last_scheduled_trigger_at;
            let icon = if manual.is_some_and(|m| scheduled.is_none_or(|s| m >= s)) {
                "↻"
            } else {
                "⏱"
            };
            html! { <div class="cell-triggered">{format!("{icon} {}", reltime(ts))}</div> }
        }
    };

    let agent_dot = if r.agent_registered {
        "handler-dot ok"
    } else {
        "handler-dot warn"
    };
    let agent_title = if r.agent_registered {
        "agent registered"
    } else {
        "agent config missing"
    };

    let next_run = next_routine_run_cell(r, props.now);

    html! {
        <tr class={if props.selected { "row-selected" } else { "" }}>
            <td class="col-select">
                <input type="checkbox" checked={props.selected} onclick={on_select}
                    aria-label={format!("Select {}", r.title)} />
            </td>
            <td>
                <div class="cell-schedule" title={r.id.clone()}>{&r.title}</div>
                if let Some(goal) = r.goal.as_ref().filter(|g| !g.is_empty()) {
                    <div class="cell-goal" title={goal.clone()}>{goal.lines().next().unwrap_or("")}</div>
                }
            </td>
            <td>
                <div class="cell-schedule">{&r.schedule}</div>
                <div class="cell-schedule-human">{cron_text}</div>
                <button
                    class={preview_btn_class}
                    title="Preview next fire times"
                    aria-label="Preview next scheduled fire times"
                    aria-expanded={(*preview_open).to_string()}
                    onclick={on_preview_toggle}
                >{"▸ fires"}</button>
                {fires_panel}
            </td>
            <td>{next_run}</td>
            <td>{last_fire}</td>
            <td>
                <span class="cell-handler" title={agent_title}>
                    <span class={agent_dot}></span>
                    {&r.agent}
                </span>
            </td>
            <td>{
                if repos == 0 {
                    html! { <span class="cell-meta">{"—"}</span> }
                } else {
                    let repo_names = r.repositories.iter().map(|rr| rr.repository.as_str()).collect::<Vec<_>>().join("\n");
                    html! { <span class="cell-meta" title={repo_names}>{format!("{repos}")}</span> }
                }
            }</td>
            <td>{
                if machines.is_empty() {
                    html! { <span class="cell-meta cell-no-machines">{"—"}</span> }
                } else {
                    let machine_names = machines.join("\n");
                    html! { <span class="cell-meta" title={machine_names}>{format!("{}", machines.len())}</span> }
                }
            }</td>
            <td>
                {
                    if r.tags.is_empty() {
                        html! { <span class="cell-meta">{"—"}</span> }
                    } else {
                        html! {
                            <span class="cell-meta" title={r.tags.join(", ")}>{ r.tags.join(", ") }</span>
                        }
                    }
                }
            </td>
            <td><span class="cell-meta" title="workbench retention for finished runs">{ format_ttl(r.ttl_secs) }</span></td>
            <td>
                <span class={routine_health(r, props.now).badge_class()}
                    title={routine_health(r, props.now).badge()}>
                    {routine_health(r, props.now).badge()}
                </span>
            </td>
            <td>
                <label class="toggle">
                    <input type="checkbox" checked={r.enabled} onchange={on_toggle} />
                    <div class="toggle-track"></div>
                </label>
            </td>
            <td><div class="cell-time">{updated}</div></td>
            <td>
                <div class="row-actions">
                    <button class="act-btn run" title="Run now" aria-label="Run now" onclick={on_trigger}>{"▶"}</button>
                    <button class="act-btn logs" onclick={on_logs}>{"LOGS"}</button>
                    <button class="act-btn flags" title="Open flags" onclick={on_flags}>
                        {"FLAGS"}
                        if r.flag_count > 0 {
                            <span class="flag-badge">{r.flag_count}</span>
                        }
                    </button>
                    <button class="act-btn edit" onclick={on_edit}>{"EDIT"}</button>
                    <button class="act-btn clone" title="Duplicate routine" aria-label="Duplicate routine" onclick={on_clone}>{"⧉"}</button>
                    <button class="act-btn del" title="Delete routine" aria-label="Delete routine" onclick={on_delete}>{"✕"}</button>
                </div>
            </td>
        </tr>
    }
}
