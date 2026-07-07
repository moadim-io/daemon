//! Month-calendar view of upcoming routine fire times.

use chrono::{Datelike, Duration, Local};
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;

use crate::schedule::{month_start, occurrences_per_day, CAL_MONTHS, GRID_CELLS, WEEKDAYS};
use crate::ToastKind;

use super::filter::is_routine_snoozed;
use super::model::Routine;

// ─── Calendar ─────────────────────────────────────────────────────────────────

#[derive(Properties, PartialEq)]
pub struct CalendarProps {
    pub routines: Vec<Routine>,
    pub loading: bool,
    /// When set, clicking a calendar chip opens the edit modal for that routine.
    #[prop_or_default]
    pub on_edit: Option<Callback<String>>,
    /// When set, enables the SUBSCRIBE button which copies the `/routines.ics` feed URL.
    #[prop_or_default]
    pub on_toast: Option<Callback<(String, ToastKind)>>,
}

/// Build the absolute URL of the routines iCalendar feed from a page origin.
fn ics_feed_url(origin: &str) -> String {
    format!("{origin}/api/v1/routines.ics")
}

#[function_component(RoutineCalendar)]
pub fn routine_calendar(props: &CalendarProps) -> Html {
    let offset = use_state(|| 0i32);

    let on_prev = {
        let offset = offset.clone();
        Callback::from(move |_: MouseEvent| offset.set(*offset - 1))
    };
    let on_next = {
        let offset = offset.clone();
        Callback::from(move |_: MouseEvent| offset.set(*offset + 1))
    };
    let on_today = {
        let offset = offset.clone();
        Callback::from(move |_: MouseEvent| offset.set(0))
    };
    let on_subscribe = props.on_toast.clone().map(|on_toast| {
        Callback::from(move |_: MouseEvent| {
            let on_toast = on_toast.clone();
            spawn_local(async move {
                let Some(window) = web_sys::window() else {
                    return;
                };
                let origin = window.location().origin().unwrap_or_default();
                let url = ics_feed_url(&origin);
                let promise = window.navigator().clipboard().write_text(&url);
                match wasm_bindgen_futures::JsFuture::from(promise).await {
                    Ok(_) => on_toast.emit(("Calendar feed URL copied".into(), ToastKind::Ok)),
                    Err(_) => on_toast.emit(("Copy failed".into(), ToastKind::Err)),
                }
            });
        })
    });

    if props.loading {
        return html! {
            <div class="table-wrap"><div class="empty"><div class="spinner"></div></div></div>
        };
    }

    let today = Local::now().date_naive();
    let first = month_start(today, *offset);
    let grid_start = first - Duration::days(first.weekday().num_days_from_sunday() as i64);

    // Accumulate per-cell chips in routine order: only enabled routines with a parseable schedule.
    // Each entry is (id, title, count, snoozed) so chips can dispatch the edit modal on click.
    let cal_now = Local::now();
    let mut cells: Vec<Vec<(String, String, u32, bool)>> = vec![Vec::new(); GRID_CELLS];
    let mut scheduled = 0usize;
    for r in props.routines.iter().filter(|r| r.enabled) {
        if let Some(counts) = occurrences_per_day(&r.schedule, grid_start) {
            scheduled += 1;
            let snoozed = is_routine_snoozed(r, cal_now);
            for (i, &c) in counts.iter().enumerate() {
                if c > 0 {
                    cells[i].push((r.id.clone(), r.title.clone(), c, snoozed));
                }
            }
        }
    }

    let month_label = format!("{} {}", CAL_MONTHS[first.month0() as usize], first.year());

    let body = if scheduled == 0 {
        html! {
            <div class="empty">
                <div class="empty-icon">{"🗓"}</div>
                <div class="empty-msg">{"NOTHING SCHEDULED"}</div>
                <div class="empty-sub">{"enabled routines with a valid schedule appear here"}</div>
            </div>
        }
    } else {
        html! {
            <>
                <div class="cal-weekdays">
                    { for WEEKDAYS.iter().map(|d| html! { <div class="cal-weekday">{*d}</div> }) }
                </div>
                <div class="cal-grid">
                    { for cells.iter().enumerate().map(|(i, hits)| {
                        let date = grid_start + Duration::days(i as i64);
                        let mut cls = String::from("cal-day");
                        if date.month() != first.month() {
                            cls.push_str(" other-month");
                        }
                        if date == today {
                            cls.push_str(" today");
                        }
                        html! {
                            <div class={cls}>
                                <div class="cal-daynum">{date.day()}</div>
                                <div class="cal-hits">
                                    { for hits.iter().take(4).map(|(id, title, count, snoozed)| {
                                        let label = if *count > 1 {
                                            format!("{title} ×{count}")
                                        } else {
                                            title.clone()
                                        };
                                        let on_chip = props.on_edit.as_ref().map(|cb| {
                                            let cb = cb.clone();
                                            let id = id.clone();
                                            Callback::from(move |_: MouseEvent| cb.emit(id.clone()))
                                        });
                                        let mut chip_cls = String::from("cal-chip");
                                        if on_chip.is_some() { chip_cls.push_str(" clickable"); }
                                        if *snoozed { chip_cls.push_str(" snoozed"); }
                                        html! { <div class={chip_cls} title={label.clone()} onclick={on_chip}>{label}</div> }
                                    }) }
                                    if hits.len() > 4 {
                                        <div class="cal-more">{format!("+{} more", hits.len() - 4)}</div>
                                    }
                                </div>
                            </div>
                        }
                    }) }
                </div>
            </>
        }
    };

    html! {
        <div class="cal-wrap">
            <div class="cal-nav">
                <button class="btn-refresh" title="Previous month" aria-label="Previous month" onclick={on_prev}>{"‹"}</button>
                <div class="cal-month">{month_label}</div>
                <button class="btn-refresh" title="Next month" aria-label="Next month" onclick={on_next}>{"›"}</button>
                <button class="btn btn-ghost btn-sm" onclick={on_today}>{"TODAY"}</button>
                if let Some(on_subscribe) = on_subscribe {
                    <button class="btn btn-ghost btn-sm" title="Copy the routines.ics feed URL"
                        onclick={on_subscribe}>{"SUBSCRIBE"}</button>
                }
            </div>
            {body}
        </div>
    }
}

#[cfg(test)]
#[path = "calendar_tests.rs"]
mod calendar_tests;
