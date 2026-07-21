/**
 * Bucketing for the heatmap's day drill-down: given a set of schedule-bearing
 * items, resolve every fire that lands on one calendar day (via
 * `lib/schedule.ts`'s `fireTimesOnDay`, shared with the routines page's own
 * day-timeline) and group them by hour. Ported from `ui/src/day_timeline.rs (removed)`'s
 * bucketing loop (that file has no host-side Rust tests to port 1:1; see
 * `dayTimelineMath.test.ts` for the sanity coverage this logic gets here
 * instead).
 */
import type { RoutineResponse } from "../../api/hooks";
import { fireTimesOnDay, CAL_MONTHS, WEEKDAYS } from "../../lib/schedule";

export { fireTimesOnDay } from "../../lib/schedule";

// ponytail: the Rust source's TimelineItem also carries an `id` so a clicked
// chip can jump to that routine's detail page. This client has no per-routine
// route yet (routes.tsx only has the /routines list), so there's nowhere to
// navigate to — dropped until that route exists.

/** One schedulable thing on the timeline: a display label and its cron schedule. */
export interface TimelineItem {
  label: string;
  schedule: string;
  /** True when this routine is currently snoozed; rendered muted. */
  snoozed: boolean;
  /** Open flag count; shown as a badge when non-zero. */
  flagCount: number;
}

/** One resolved fire event inside a single hour bucket. */
export interface BucketEntry {
  time: Date;
  label: string;
  snoozed: boolean;
  flagCount: number;
}

/** Bucket every item's fire times on `day` into 24 hour rows, each sorted
 * chronologically. */
export function bucketDayFires(items: TimelineItem[], day: Date): BucketEntry[][] {
  const buckets: BucketEntry[][] = Array.from({ length: 24 }, () => []);
  for (const item of items) {
    for (const time of fireTimesOnDay(item.schedule, day)) {
      const bucket = buckets[time.getHours()];
      bucket?.push({
        time,
        label: item.label,
        snoozed: item.snoozed,
        flagCount: item.flagCount,
      });
    }
  }
  for (const bucket of buckets) bucket.sort((a, b) => a.time.getTime() - b.time.getTime());
  return buckets;
}

/** Whether scheduled fires are currently suppressed for `routine`, at `now`.
 * Ported from `ui/src/overview.rs (removed)`'s `is_snoozed`. */
function isRoutineSnoozed(routine: RoutineResponse, now: Date): boolean {
  const until = routine.snoozed_until;
  const snoozed = until != null && until > Math.floor(now.getTime() / 1000);
  return snoozed || (routine.skip_runs ?? 0) > 0;
}

/** Map enabled routines onto the day-timeline's item shape. */
export function timelineItemsOf(routines: RoutineResponse[], now: Date): TimelineItem[] {
  return routines
    .filter((r) => r.enabled)
    .map((r) => ({
      label: r.title,
      schedule: r.schedule,
      snoozed: isRoutineSnoozed(r, now),
      flagCount: r.flag_count,
    }));
}

/** `"SUN · JUN 21 2026"`-style label for the day-timeline header. */
export function dayTimelineLabel(day: Date): string {
  const weekday = WEEKDAYS[day.getDay()];
  const month = CAL_MONTHS[day.getMonth()]?.slice(0, 3);
  return `${weekday} · ${month} ${day.getDate()} ${day.getFullYear()}`;
}
