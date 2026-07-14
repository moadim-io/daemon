import { parseExpression } from "cron-parser";
import { normalizeCron } from "./cronUtils";

const MONTHS = [
  "Jan",
  "Feb",
  "Mar",
  "Apr",
  "May",
  "Jun",
  "Jul",
  "Aug",
  "Sep",
  "Oct",
  "Nov",
  "Dec",
] as const;

/** Parses `schedule` primed at `currentDate`, or `undefined` if empty/invalid. Exported so
 * other schedule-bearing views (e.g. the heatmap/day-timeline pages) can reuse this cron-parser
 * wiring instead of duplicating it. */
export function parseSchedule(schedule: string, currentDate: Date) {
  const s = normalizeCron(schedule);
  if (s === "") return undefined;
  try {
    return parseExpression(s, { currentDate });
  } catch {
    return undefined;
  }
}

/** The next fire time strictly after `now`, or `undefined` if invalid/empty/never fires again. */
export function nextFireAfter(schedule: string, now: Date): Date | undefined {
  const cron = parseSchedule(schedule, now);
  if (!cron?.hasNext()) return undefined;
  return cron.next().toDate();
}

/** The next `n` fire times strictly after `now`. Fewer than `n` when the schedule runs out. */
export function nextFires(schedule: string, now: Date, n: number): Date[] {
  const cron = parseSchedule(schedule, now);
  if (!cron) return [];
  const out: Date[] = [];
  while (out.length < n && cron.hasNext()) {
    out.push(cron.next().toDate());
  }
  return out;
}

/** `true` when `schedule`'s next fire lands within `windowMs` of `now`. */
export function firesWithin(schedule: string, now: Date, windowMs: number): boolean {
  const then = nextFireAfter(schedule, now);
  return then !== undefined && then.getTime() - now.getTime() <= windowMs;
}

/** Relative countdown from `now` to `then`: "in 5m", "in 2h 10m", "in 3d", "in <1m", or "now". */
export function fmtUntil(now: Date, then: Date): string {
  const secs = Math.floor((then.getTime() - now.getTime()) / 1000);
  if (secs <= 0) return "now";
  const mins = Math.floor(secs / 60);
  if (mins < 1) return "in <1m";
  if (mins < 60) return `in ${mins}m`;
  const hours = Math.floor(mins / 60);
  if (hours < 24) {
    const rem = mins % 60;
    return rem === 0 ? `in ${hours}h` : `in ${hours}h ${rem}m`;
  }
  const days = Math.floor(hours / 24);
  return `in ${days}d`;
}

/** Truncates `d` to its local calendar day (midnight). Exported for reuse by other
 * day-bucketing views (heatmap/day-timeline). */
export function dateOnly(d: Date): Date {
  return new Date(d.getFullYear(), d.getMonth(), d.getDate());
}

/** Absolute fire time relative to `now`'s calendar day: "14:30", "tomorrow 09:00", or "Jun 24, 09:00". */
export function fmtWhen(now: Date, then: Date): string {
  const hm = `${String(then.getHours()).padStart(2, "0")}:${String(then.getMinutes()).padStart(2, "0")}`;
  const days = Math.round((dateOnly(then).getTime() - dateOnly(now).getTime()) / 86_400_000);
  if (days === 0) return hm;
  if (days === 1) return `tomorrow ${hm}`;
  return `${MONTHS[then.getMonth()]} ${then.getDate()}, ${hm}`;
}

// ─── Calendar grid utilities (shared by the Heatmap and Routines calendar views) ──

export const WEEKDAYS = ["SUN", "MON", "TUE", "WED", "THU", "FRI", "SAT"] as const;

export const CAL_MONTHS = [
  "JANUARY",
  "FEBRUARY",
  "MARCH",
  "APRIL",
  "MAY",
  "JUNE",
  "JULY",
  "AUGUST",
  "SEPTEMBER",
  "OCTOBER",
  "NOVEMBER",
  "DECEMBER",
] as const;

/** Cells in the month grid: 6 weeks x 7 days, always, so the layout never reflows. */
export const GRID_CELLS = 42;

/** Upper bound on fire-time iterations per schedule across the visible grid. */
export const MAX_OCCURRENCES = 4_000;

/** First day of the month `offsetMonths` away from the month containing `today`. */
export function monthStart(today: Date, offsetMonths: number): Date {
  const total = today.getFullYear() * 12 + today.getMonth() + offsetMonths;
  const year = Math.floor(total / 12);
  const month = ((total % 12) + 12) % 12;
  return new Date(year, month, 1);
}

/** Upper bound on fire-time iterations per schedule for one day. An every-minute schedule fires
 * 1440 times/day; this leaves headroom while bounding cost on pathological inputs. */
const MAX_FIRES_PER_DAY = 2_000;

/** All fire times for `schedule` that fall on `day`, in chronological order. Shared by the
 * heatmap and routines day-timeline views so they resolve "what fires today" identically instead
 * of maintaining separate cron-parsing copies that can drift apart. */
export function fireTimesOnDay(schedule: string, day: Date): Date[] {
  const dayStart = dateOnly(day);
  // Step back one second so an occurrence exactly at midnight counts as part of the day.
  const start = new Date(dayStart.getTime() - 1_000);
  const cron = parseSchedule(schedule, start);
  if (!cron) return [];
  const dayEnd = new Date(dayStart.getTime() + 86_400_000);
  const out: Date[] = [];
  for (let i = 0; i < MAX_FIRES_PER_DAY && cron.hasNext(); i++) {
    const dt = cron.next().toDate();
    if (dt.getTime() < dayStart.getTime()) continue;
    if (dt.getTime() >= dayEnd.getTime()) break;
    out.push(dt);
  }
  return out;
}

/** Fire counts per grid cell for `schedule` over `[gridStart, gridStart + 42 days)`. */
export function occurrencesPerDay(schedule: string, gridStart: Date): number[] | undefined {
  const start = new Date(gridStart.getTime() - 1000);
  const cron = parseSchedule(schedule, start);
  if (!cron) return undefined;
  const counts = new Array<number>(GRID_CELLS).fill(0);
  const gridStartDay = dateOnly(gridStart).getTime();
  for (let i = 0; i < MAX_OCCURRENCES && cron.hasNext(); i++) {
    const dt = cron.next().toDate();
    const day = Math.floor((dateOnly(dt).getTime() - gridStartDay) / 86_400_000);
    if (day < 0) continue;
    if (day >= GRID_CELLS) break;
    counts[day] = (counts[day] ?? 0) + 1;
  }
  return counts;
}
