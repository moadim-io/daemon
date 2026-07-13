//! Routine run-history page.
//!
//! Lists every run workbench kept for a routine (newest first), each with its status (derived
//! server-side from the exit-code file the launch command writes and the tmux session's liveness)
//! and a way to view that specific run's log — unlike the LOGS page, which only ever shows the
//! newest run.

use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;

use crate::log_viewer::LogViewer;

use super::model::{api_run_log, api_runs, RunStatus, RunSummary};

/// CSS class for a run's status badge.
pub(crate) fn run_status_class(status: RunStatus) -> &'static str {
    match status {
        RunStatus::Running => "run-status running",
        RunStatus::Success => "run-status success",
        RunStatus::Failed => "run-status failed",
        RunStatus::Unknown => "run-status unknown",
    }
}

/// Display label for a run's status badge.
pub(crate) fn run_status_label(status: RunStatus) -> &'static str {
    match status {
        RunStatus::Running => "RUNNING",
        RunStatus::Success => "SUCCESS",
        RunStatus::Failed => "FAILED",
        RunStatus::Unknown => "UNKNOWN",
    }
}

/// Format the wall-clock duration between a run's start and finish as `"<n>s"`/`"<n>m"`/`"<n>h <n>m"`.
pub(crate) fn fmt_run_duration(started_at: u64, finished_at: u64) -> String {
    let secs = finished_at.saturating_sub(started_at);
    if secs < 60 {
        format!("{secs}s")
    } else if secs < 3_600 {
        format!("{}m", secs / 60)
    } else {
        format!("{}h {}m", secs / 3_600, (secs % 3_600) / 60)
    }
}

/// Format a humanized countdown to when a finished run's workbench is due to be reaped, given
/// the current time and the run's `retention_expires_at`. Once the deadline has passed this
/// reads `"expired"` rather than a negative countdown — cleanup runs on its own interval, so a
/// due run can still briefly be visible.
pub(crate) fn fmt_retention(now: u64, expires_at: u64) -> String {
    if now >= expires_at {
        return "expired".to_string();
    }
    let secs = expires_at - now;
    if secs < 60 {
        "expires in <1m".to_string()
    } else if secs < 3_600 {
        format!("expires in {}m", secs / 60)
    } else {
        format!("expires in {}h {}m", secs / 3_600, (secs % 3_600) / 60)
    }
}

#[derive(Properties, PartialEq)]
pub struct HistoryProps {
    pub id: String,
    pub title: String,
    pub on_back: Callback<()>,
}

#[function_component(RoutineHistory)]
pub fn routine_history(props: &HistoryProps) -> Html {
    let runs: UseStateHandle<Vec<RunSummary>> = use_state(Vec::new);
    let loading = use_state(|| true);
    let err: UseStateHandle<Option<String>> = use_state(|| None);
    let selected: UseStateHandle<Option<String>> = use_state(|| None);
    let log_content: UseStateHandle<Option<String>> = use_state(|| None);
    let log_loading = use_state(|| false);
    let log_err: UseStateHandle<Option<String>> = use_state(|| None);
    let updated_at: UseStateHandle<f64> = use_state(|| 0.0);

    let load = {
        let id = props.id.clone();
        let runs = runs.clone();
        let loading = loading.clone();
        let err = err.clone();
        let updated_at = updated_at.clone();
        move || {
            let id = id.clone();
            let runs = runs.clone();
            let loading = loading.clone();
            let err = err.clone();
            let updated_at = updated_at.clone();
            loading.set(true);
            spawn_local(async move {
                match api_runs(&id).await {
                    Ok(list) => {
                        runs.set(list);
                        err.set(None);
                        updated_at.set(js_sys::Date::now());
                    }
                    Err(e) => err.set(Some(e)),
                }
                loading.set(false);
            });
        }
    };

    {
        let load = load.clone();
        use_effect_with(props.id.clone(), move |_| {
            load();
        });
    }

    let on_back = {
        let cb = props.on_back.clone();
        Callback::from(move |_: MouseEvent| cb.emit(()))
    };
    let on_refresh = {
        let load = load.clone();
        Callback::from(move |_: MouseEvent| load())
    };

    {
        let id = props.id.clone();
        let log_content = log_content.clone();
        let log_loading = log_loading.clone();
        let log_err = log_err.clone();
        use_effect_with(((*selected).clone(), id), move |(workbench, id)| {
            let id = id.clone();
            match workbench.clone() {
                None => {
                    log_content.set(None);
                    log_err.set(None);
                }
                Some(workbench) => {
                    log_loading.set(true);
                    spawn_local(async move {
                        match api_run_log(&id, &workbench).await {
                            Ok(text) => {
                                log_content.set(Some(text));
                                log_err.set(None);
                            }
                            Err(e) => log_err.set(Some(e)),
                        }
                        log_loading.set(false);
                    });
                }
            }
        });
    }

    let now_secs = (js_sys::Date::now() / 1000.0) as u64;
    let rows = runs.iter().map(|run| {
        let started = crate::reltime(run.started_at);
        let started_title = format!("{} · {}", run.workbench, crate::abstime(run.started_at));
        let duration = run.finished_at.map(|f| fmt_run_duration(run.started_at, f));
        let exit_code = run
            .exit_code
            .map_or_else(|| "—".to_string(), |c| c.to_string());
        let retention = run
            .retention_expires_at
            .map(|expires_at| fmt_retention(now_secs, expires_at));
        let workbench = run.workbench.clone();
        let is_selected = selected.as_deref() == Some(workbench.as_str());
        let on_view = {
            let selected = selected.clone();
            let workbench = workbench.clone();
            Callback::from(move |_: MouseEvent| {
                selected.set(if selected.as_deref() == Some(workbench.as_str()) {
                    None
                } else {
                    Some(workbench.clone())
                });
            })
        };
        html! {
            <tr key={workbench.clone()} class={if is_selected { "row-selected" } else { "" }}>
                <td><div class="cell-time" title={started_title}>{started}</div></td>
                <td><span class={run_status_class(run.status)}>{run_status_label(run.status)}</span></td>
                <td>{duration.unwrap_or_else(|| "—".to_string())}</td>
                <td>{exit_code}</td>
                <td><span class="cell-meta">{retention.unwrap_or_else(|| "—".to_string())}</span></td>
                <td>
                    <button class="act-btn logs" onclick={on_view}>
                        { if is_selected { "HIDE LOG" } else { "VIEW LOG" } }
                    </button>
                </td>
            </tr>
        }
    }).collect::<Html>();

    let body = if *loading {
        html! { <div class="table-wrap"><div class="empty"><div class="spinner"></div></div></div> }
    } else if let Some(e) = &*err {
        html! { <div class="logs-error">{format!("Error: {e}")}</div> }
    } else if runs.is_empty() {
        html! {
            <div class="table-wrap">
                <div class="empty">
                    <div class="empty-icon">{"⧗"}</div>
                    <div class="empty-msg">{"NO RUNS YET"}</div>
                </div>
            </div>
        }
    } else {
        html! {
            <div class="table-wrap">
                <table>
                    <thead>
                        <tr>
                            <th>{"STARTED"}</th>
                            <th>{"STATUS"}</th>
                            <th>{"DURATION"}</th>
                            <th>{"EXIT CODE"}</th>
                            <th>{"RETENTION"}</th>
                            <th></th>
                        </tr>
                    </thead>
                    <tbody>{rows}</tbody>
                </table>
            </div>
        }
    };

    let freshness = if *updated_at > 0.0 {
        let secs = ((js_sys::Date::now() - *updated_at).max(0.0) / 1000.0) as u64;
        crate::refresh::fmt_freshness(secs)
    } else {
        String::new()
    };

    html! {
        <main class="logs-page">
            <div class="page-hd">
                <button class="btn btn-ghost btn-sm" onclick={on_back}>{"← BACK"}</button>
                <div class="page-title">{format!("HISTORY / {}", props.title)}</div>
                <span class="page-freshness">{freshness}</span>
                <button class="btn-refresh" title="Refresh" aria-label="Refresh" onclick={on_refresh}>{"↻"}</button>
            </div>
            {body}
            if selected.is_some() {
                <LogViewer
                    content={(*log_content).clone()}
                    loading={*log_loading}
                    err={(*log_err).clone()}
                />
            }
        </main>
    }
}

#[cfg(test)]
#[path = "history_tests.rs"]
mod history_tests;
