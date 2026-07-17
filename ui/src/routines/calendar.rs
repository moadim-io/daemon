//! Month-calendar view of upcoming routine fire times.

use chrono::{DateTime, Datelike, Duration, Local, NaiveDate};
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;

use crate::schedule::{
    fires_on_day, month_start, occurrences_per_day, CAL_MONTHS, GRID_CELLS, WEEKDAYS,
};
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
    /// When set, the day-detail popover's "▶ RUN" button triggers a routine immediately.
    #[prop_or_default]
    pub on_trigger: Option<Callback<String>>,
}

/// Every enabled routine's fire times on `date`, as `(routine id, title, "HH:MM", snoozed)`,
/// sorted chronologically. `snoozed` mirrors [`is_routine_snoozed`] so the popover can flag a
/// row whose scheduled fire will actually be silently skipped (`snoozed_until`/`skip_runs`) —
/// the same signal the month grid's chips already dim, which this list previously dropped.
/// Pure and host-testable — see `calendar_tests.rs`.
pub(crate) fn day_fire_rows(
    routines: &[Routine],
    date: NaiveDate,
    now: DateTime<Local>,
) -> Vec<(String, String, String, bool)> {
    let mut rows: Vec<(String, String, String, bool)> = routines
        .iter()
        .filter(|r| r.enabled)
        .flat_map(|r| {
            let snoozed = is_routine_snoozed(r, now);
            fires_on_day(&r.schedule, date)
                .into_iter()
                .map(move |hm| (r.id.clone(), r.title.clone(), hm, snoozed))
        })
        .collect();
    rows.sort_by(|a, b| a.2.cmp(&b.2));
    rows
}

/// Format a date for the day-detail popover title, e.g. "JUN 21, 2026".
fn day_title(date: NaiveDate) -> String {
    format!(
        "{} {}, {}",
        CAL_MONTHS[date.month0() as usize],
        date.day(),
        date.year()
    )
}

/// Build the absolute URL of the routines iCalendar feed from a page origin.
fn ics_feed_url(origin: &str) -> String {
    format!("{origin}/api/v1/routines.ics")
}

#[function_component(RoutineCalendar)]
pub fn routine_calendar(props: &CalendarProps) -> Html {
    let offset = use_state(|| 0i32);
    let selected_day = use_state(|| None::<NaiveDate>);

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
    let grid_start = first - Duration::days(i64::from(first.weekday().num_days_from_sunday()));

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
                        let on_open_day = {
                            let selected_day = selected_day.clone();
                            Callback::from(move |_: MouseEvent| selected_day.set(Some(date)))
                        };
                        html! {
                            <div class={cls}>
                                <div class="cal-daynum clickable" title="Show fire times for this day" onclick={on_open_day}>{date.day()}</div>
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

    let day_popover = if let Some(day) = *selected_day {
        let rows = day_fire_rows(&props.routines, day, cal_now);
        let on_close = {
            let selected_day = selected_day.clone();
            Callback::from(move |_: MouseEvent| selected_day.set(None))
        };
        html! {
            <div class="overlay open">
                <div class="modal">
                    <div class="modal-hd">
                        <div class="modal-title">{day_title(day)}</div>
                        <button class="modal-x" title="Close" aria-label="Close" onclick={on_close.clone()}>{"✕"}</button>
                    </div>
                    <div class="modal-body day-fires">
                        if rows.is_empty() {
                            <div class="empty-sub">{"Nothing scheduled this day"}</div>
                        } else {
                            { for rows.iter().map(|(id, title, hm, snoozed)| {
                                let on_run = props.on_trigger.as_ref().map(|cb| {
                                    let cb = cb.clone();
                                    let id = id.clone();
                                    Callback::from(move |_: MouseEvent| cb.emit(id.clone()))
                                });
                                let mut row_cls = String::from("day-fire-row");
                                if *snoozed { row_cls.push_str(" snoozed"); }
                                html! {
                                    <div class={row_cls}>
                                        <span class="day-fire-time">{hm}</span>
                                        <span class="day-fire-title">{title}</span>
                                        if *snoozed {
                                            <span class="health-badge snoozed" title="This fire will be silently skipped while the routine is snoozed">{"SNOOZED"}</span>
                                        }
                                        if let Some(on_run) = on_run {
                                            <button class="btn btn-ghost btn-sm" title="Run now" onclick={on_run}>{"▶ RUN"}</button>
                                        }
                                    </div>
                                }
                            }) }
                        }
                    </div>
                    <div class="modal-ft">
                        <button class="btn btn-ghost btn-sm" onclick={on_close}>{"CLOSE"}</button>
                    </div>
                </div>
            </div>
        }
    } else {
        html! {}
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
            {day_popover}
        </div>
    }
}

#[cfg(test)]
#[path = "calendar_tests.rs"]
mod calendar_tests;
