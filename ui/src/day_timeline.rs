//! Shared "day" calendar view: a single day's fire times laid out on a scrollable
//! 24-hour timeline. Used by the routines page.
//!
//! The caller maps its own items (routines) to [`TimelineItem`]s — a label plus a
//! cron schedule — and this component computes each item's fire times for the
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

/// Zoom levels: pixel height of one hour row. The first level keeps the original
/// compact, chip-wrapping layout; the rest grow the hour tall enough to lay fire
/// times out by their exact minute against quarter-hour guide lines, so you can
/// read sub-hour timing instead of a single wrapped bucket.
const ZOOM_LEVELS: [i32; 4] = [40, 140, 300, 600];

/// One schedulable thing on the timeline: a display label and its cron schedule.
#[derive(Clone, PartialEq, Eq)]
pub struct TimelineItem {
    /// Optional entity ID emitted by `on_click` when a chip is clicked.
    pub id: Option<String>,
    pub label: String,
    pub schedule: String,
    /// True when this routine is currently snoozed; the chip is rendered muted.
    pub snoozed: bool,
    /// Open flag count; shown as a badge when non-zero.
    pub flag_count: usize,
}

/// One resolved fire event inside a single hour bucket.
struct BucketEntry {
    time: NaiveTime,
    label: String,
    id: Option<String>,
    snoozed: bool,
    flag_count: usize,
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
    /// When set, clicking a chip that has an `id` calls this with that id.
    #[prop_or_default]
    pub on_click: Option<Callback<String>>,
}

#[function_component(DayTimeline)]
pub fn day_timeline(props: &DayTimelineProps) -> Html {
    // Day offset from today; negative = past, positive = future.
    let offset = use_state(|| 0i64);
    // Zoom level index into ZOOM_LEVELS; 0 = compact, higher = deeper into the hour.
    let zoom = use_state(|| 0usize);
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
    let on_zoom_in = {
        let zoom = zoom.clone();
        Callback::from(move |_: MouseEvent| zoom.set((*zoom + 1).min(ZOOM_LEVELS.len() - 1)))
    };
    let on_zoom_out = {
        let zoom = zoom.clone();
        Callback::from(move |_: MouseEvent| zoom.set(zoom.saturating_sub(1)))
    };

    // Scroll the current hour into view whenever we land on today.
    {
        let now_ref = now_ref.clone();
        use_effect_with((*offset, *zoom), move |_| {
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
    let mut buckets: Vec<Vec<BucketEntry>> = (0..24).map(|_| Vec::new()).collect();
    let mut total = 0usize;
    for it in &props.items {
        for t in fire_times(&it.schedule, day) {
            buckets[t.hour() as usize].push(BucketEntry {
                time: t,
                label: it.label.clone(),
                id: it.id.clone(),
                snoozed: it.snoozed,
                flag_count: it.flag_count,
            });
            total += 1;
        }
    }
    for b in &mut buckets {
        b.sort_by_key(|e| e.time);
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
        // Level 0 keeps the compact wrap layout; deeper levels switch to a
        // minute-positioned timeline with quarter-hour guide lines.
        let detailed = *zoom > 0;
        let hour_px = ZOOM_LEVELS[*zoom];
        let scroll_cls = if detailed {
            "day-scroll detail"
        } else {
            "day-scroll"
        };
        html! {
            <div class={scroll_cls} style={format!("--dh:{hour_px}px")}>
                { for (0..24usize).map(|h| {
                    let slot = &buckets[h];
                    let mut cls = String::from("day-hour");
                    if h == now_hour {
                        cls.push_str(" now");
                    }
                    let row_ref = if h == now_hour { now_ref.clone() } else { NodeRef::default() };
                    let label = if detailed {
                        html! {
                            <div class="day-hour-label">
                                <span>{format!("{h:02}:00")}</span>
                                <span class="qt">{format!("{h:02}:15")}</span>
                                <span class="qt">{format!("{h:02}:30")}</span>
                                <span class="qt">{format!("{h:02}:45")}</span>
                                <span class="qt-end"></span>
                            </div>
                        }
                    } else {
                        html! { <div class="day-hour-label">{format!("{h:02}:00")}</div> }
                    };
                    html! {
                        <div class={cls} ref={row_ref}>
                            {label}
                            <div class="day-hour-slot">
                                { for slot.iter().map(|e| {
                                    let t = e.time;
                                    let frac = (t.minute() as f32 + t.second() as f32 / 60.0) / 60.0;
                                    let style = if detailed {
                                        Some(format!("top:{:.3}%", frac * 100.0))
                                    } else {
                                        None
                                    };
                                    let on_chip = props.on_click.as_ref().zip(e.id.as_ref()).map(|(cb, item_id)| {
                                        let cb = cb.clone();
                                        let item_id = item_id.clone();
                                        Callback::from(move |_: MouseEvent| cb.emit(item_id.clone()))
                                    });
                                    let mut chip_cls = String::from("day-chip");
                                    if on_chip.is_some() { chip_cls.push_str(" clickable"); }
                                    if e.snoozed { chip_cls.push_str(" snoozed"); }
                                    let flag_count = e.flag_count;
                                    html! {
                                        <div class={chip_cls} style={style} title={e.label.clone()} onclick={on_chip}>
                                            <span class="day-chip-time">{format!("{:02}:{:02}", t.hour(), t.minute())}</span>
                                            <span class="day-chip-label">{e.label.clone()}</span>
                                            if flag_count > 0 {
                                                <span class="day-chip-flag">{format!("⚑{flag_count}")}</span>
                                            }
                                        </div>
                                    }
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
                <div class="day-zoom">
                    <button class="btn-refresh" title="Zoom out" aria-label="Zoom out"
                        disabled={*zoom == 0} onclick={on_zoom_out}>{"−"}</button>
                    <span class="day-zoom-level">{format!("{}×", *zoom + 1)}</span>
                    <button class="btn-refresh" title="Zoom into the hour" aria-label="Zoom in"
                        disabled={*zoom == ZOOM_LEVELS.len() - 1} onclick={on_zoom_in}>{"+"}</button>
                </div>
                <button class="btn btn-ghost btn-sm" onclick={on_today}>{"TODAY"}</button>
            </div>
            {body}
        </div>
    }
}
