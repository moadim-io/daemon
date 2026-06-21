//! Shared "day" calendar view: a single day's fire times laid out on a scrollable
//! 24-hour timeline. Used by both the routines and cron-jobs pages.
//!
//! The caller maps its own items (routines, cron jobs) to [`TimelineItem`]s — a label
//! plus a cron schedule — and this component computes each item's fire times for the
//! selected day and buckets them into hour rows.

use chrono::{Datelike, Duration, Local, NaiveDate, NaiveTime, TimeZone, Timelike};
use web_sys::Element;
use yew::prelude::*;

use crate::parse_cron;

/// Upper bound on fire-time iterations per item for one day. An every-minute schedule
/// fires 1440 times/day; this leaves headroom while bounding cost on pathological inputs.
const MAX_FIRES: usize = 2000;

const WEEKDAYS: [&str; 7] = ["SUN", "MON", "TUE", "WED", "THU", "FRI", "SAT"];
const MONTHS: [&str; 12] = [
    "JAN", "FEB", "MAR", "APR", "MAY", "JUN", "JUL", "AUG", "SEP", "OCT", "NOV", "DEC",
];

/// One schedulable thing on the timeline: a display label and its cron schedule.
#[derive(Clone, PartialEq)]
pub struct TimelineItem {
    pub label: String,
    pub schedule: String,
}

/// All fire times for `schedule` that fall on `day`, in chronological order.
fn fire_times(schedule: &str, day: NaiveDate) -> Vec<NaiveTime> {
    let Some(cron) = parse_cron(schedule) else {
        return Vec::new();
    };
    let Some(start_naive) = day.and_hms_opt(0, 0, 0) else {
        return Vec::new();
    };
    // Step back one second so an occurrence exactly at midnight counts as part of the day.
    let Some(start) = Local
        .from_local_datetime(&start_naive)
        .earliest()
        .and_then(|t| t.checked_sub_signed(Duration::seconds(1)))
    else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for dt in cron.iter_after(start).take(MAX_FIRES) {
        let d = dt.date_naive();
        if d < day {
            continue;
        }
        if d > day {
            break;
        }
        out.push(dt.time());
    }
    out
}

#[derive(Properties, PartialEq)]
pub struct DayTimelineProps {
    pub items: Vec<TimelineItem>,
    pub loading: bool,
}

#[function_component(DayTimeline)]
pub fn day_timeline(props: &DayTimelineProps) -> Html {
    // Day offset from today; negative = past, positive = future.
    let offset = use_state(|| 0i64);
    // Ref on the "now" hour row so we can scroll it into view when viewing today.
    let now_ref = use_node_ref();

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

    // Scroll the current hour into view whenever we land on today.
    {
        let now_ref = now_ref.clone();
        use_effect_with(*offset, move |_| {
            if let Some(el) = now_ref.cast::<Element>() {
                el.scroll_into_view_with_bool(true);
            }
            || ()
        });
    }

    if props.loading {
        return html! {
            <div class="day-wrap"><div class="empty"><div class="spinner"></div></div></div>
        };
    }

    let today = Local::now().date_naive();
    let day = today + Duration::days(*offset);
    let now_hour = if *offset == 0 {
        Local::now().hour() as usize
    } else {
        usize::MAX
    };

    // Bucket every item's fire times into the hour rows.
    let mut buckets: Vec<Vec<(NaiveTime, String)>> = vec![Vec::new(); 24];
    let mut total = 0usize;
    for it in props.items.iter() {
        for t in fire_times(&it.schedule, day) {
            buckets[t.hour() as usize].push((t, it.label.clone()));
            total += 1;
        }
    }
    for b in buckets.iter_mut() {
        b.sort_by_key(|(t, _)| *t);
    }

    let date_label = format!(
        "{} · {} {} {}",
        WEEKDAYS[day.weekday().num_days_from_sunday() as usize],
        MONTHS[day.month0() as usize],
        day.day(),
        day.year()
    );

    let body = if total == 0 {
        html! {
            <div class="empty">
                <div class="empty-icon">{"🗓"}</div>
                <div class="empty-msg">{"NOTHING SCHEDULED"}</div>
                <div class="empty-sub">{"no fire times on this day"}</div>
            </div>
        }
    } else {
        html! {
            <div class="day-scroll">
                { for (0..24usize).map(|h| {
                    let slot = &buckets[h];
                    let mut cls = String::from("day-hour");
                    if h == now_hour {
                        cls.push_str(" now");
                    }
                    let row_ref = if h == now_hour { now_ref.clone() } else { NodeRef::default() };
                    html! {
                        <div class={cls} ref={row_ref}>
                            <div class="day-hour-label">{format!("{h:02}:00")}</div>
                            <div class="day-hour-slot">
                                { for slot.iter().map(|(t, label)| html! {
                                    <div class="day-chip" title={label.clone()}>
                                        <span class="day-chip-time">{format!("{:02}:{:02}", t.hour(), t.minute())}</span>
                                        <span class="day-chip-label">{label.clone()}</span>
                                    </div>
                                }) }
                            </div>
                        </div>
                    }
                }) }
            </div>
        }
    };

    html! {
        <div class="day-wrap">
            <div class="day-nav">
                <button class="btn-refresh" title="Previous day" aria-label="Previous day" onclick={on_prev}>{"‹"}</button>
                <div class="day-date">{date_label}</div>
                <button class="btn-refresh" title="Next day" aria-label="Next day" onclick={on_next}>{"›"}</button>
                <button class="btn btn-ghost btn-sm" onclick={on_today}>{"TODAY"}</button>
            </div>
            {body}
        </div>
    }
}
