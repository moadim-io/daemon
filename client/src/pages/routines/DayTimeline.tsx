import { parseExpression } from "cron-parser";
import { useEffect, useRef, useState } from "react";
import { normalizeCron } from "../../lib/cronUtils";
import { CAL_MONTHS, WEEKDAYS } from "../../lib/schedule";

/** Upper bound on fire-time iterations per item for one day. */
const MAX_FIRES = 2_000;

/** Pixel height of one hour row per zoom level; index 0 is the compact, chip-wrapping layout. */
const ZOOM_LEVELS = [40, 140, 300, 600];

const MONTHS_SHORT = CAL_MONTHS.map((m) => m.slice(0, 3));

function dateOnly(d: Date): Date {
  const out = new Date(d);
  out.setHours(0, 0, 0, 0);
  return out;
}

/** All fire times for `schedule` that fall on `day`, in chronological order. */
function firesOnDay(schedule: string, day: Date): Date[] {
  const s = normalizeCron(schedule);
  if (s === "") return [];
  const dayStart = dateOnly(day);
  const dayEnd = new Date(dayStart);
  dayEnd.setDate(dayEnd.getDate() + 1);
  let cron;
  try {
    cron = parseExpression(s, { currentDate: new Date(dayStart.getTime() - 1_000) });
  } catch {
    return [];
  }
  const out: Date[] = [];
  for (let i = 0; i < MAX_FIRES && cron.hasNext(); i++) {
    const dt = cron.next().toDate();
    if (dt >= dayEnd) break;
    if (dt >= dayStart) out.push(dt);
  }
  return out;
}

export interface TimelineItem {
  id?: string;
  label: string;
  schedule: string;
  /** Rendered muted when true. */
  snoozed: boolean;
  flagCount: number;
}

export interface DayTimelineProps {
  items: TimelineItem[];
  loading: boolean;
  onClick?: (id: string) => void;
}

interface BucketEntry {
  time: Date;
  label: string;
  id: string | undefined;
  snoozed: boolean;
  flagCount: number;
}

/** Single day's fire times laid out on a scrollable 24-hour timeline. */
export function DayTimeline({ items, loading, onClick }: DayTimelineProps) {
  const [offset, setOffset] = useState(0);
  const [zoom, setZoom] = useState(0);
  const nowRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    nowRef.current?.scrollIntoView({ block: "center" });
  }, [offset, zoom]);

  if (loading) {
    return (
      <div className="day-wrap">
        <div className="empty">
          <div className="spinner" />
        </div>
      </div>
    );
  }

  const today = dateOnly(new Date());
  const day = new Date(today);
  day.setDate(day.getDate() + offset);
  const nowHour = offset === 0 ? new Date().getHours() : -1;

  const buckets: BucketEntry[][] = Array.from({ length: 24 }, () => []);
  let total = 0;
  for (const it of items) {
    for (const t of firesOnDay(it.schedule, day)) {
      buckets[t.getHours()]?.push({ time: t, label: it.label, id: it.id, snoozed: it.snoozed, flagCount: it.flagCount });
      total++;
    }
  }
  for (const b of buckets) b.sort((a, b2) => a.time.getTime() - b2.time.getTime());

  const dateLabel = `${WEEKDAYS[day.getDay()]} · ${MONTHS_SHORT[day.getMonth()]} ${day.getDate()} ${day.getFullYear()}`;

  const detailed = zoom > 0;
  const hourPx = ZOOM_LEVELS[zoom] ?? ZOOM_LEVELS[0];

  return (
    <div className="day-wrap">
      <div className="day-nav">
        <button type="button" className="btn-refresh" title="Previous day" aria-label="Previous day" onClick={() => setOffset((o) => o - 1)}>
          ‹
        </button>
        <div className="day-date">{dateLabel}</div>
        <button type="button" className="btn-refresh" title="Next day" aria-label="Next day" onClick={() => setOffset((o) => o + 1)}>
          ›
        </button>
        <div className="day-zoom">
          <button
            type="button"
            className="btn-refresh"
            title="Zoom out"
            aria-label="Zoom out"
            disabled={zoom === 0}
            onClick={() => setZoom((z) => Math.max(0, z - 1))}
          >
            −
          </button>
          <span className="day-zoom-level">{zoom + 1}×</span>
          <button
            type="button"
            className="btn-refresh"
            title="Zoom into the hour"
            aria-label="Zoom in"
            disabled={zoom === ZOOM_LEVELS.length - 1}
            onClick={() => setZoom((z) => Math.min(ZOOM_LEVELS.length - 1, z + 1))}
          >
            +
          </button>
        </div>
        <button type="button" className="btn btn-ghost btn-sm" onClick={() => setOffset(0)}>
          TODAY
        </button>
      </div>
      {total === 0 ? (
        <div className="empty">
          <div className="empty-icon">🗓</div>
          <div className="empty-msg">NOTHING SCHEDULED</div>
          <div className="empty-sub">no fire times on this day</div>
        </div>
      ) : (
        <div className={detailed ? "day-scroll detail" : "day-scroll"} style={{ ["--dh" as string]: `${hourPx}px` }}>
          {buckets.map((slot, h) => (
            <div className={h === nowHour ? "day-hour now" : "day-hour"} ref={h === nowHour ? nowRef : undefined} key={h}>
              {detailed ? (
                <div className="day-hour-label">
                  <span>{String(h).padStart(2, "0")}:00</span>
                  <span className="qt">{String(h).padStart(2, "0")}:15</span>
                  <span className="qt">{String(h).padStart(2, "0")}:30</span>
                  <span className="qt">{String(h).padStart(2, "0")}:45</span>
                  <span className="qt-end" />
                </div>
              ) : (
                <div className="day-hour-label">{String(h).padStart(2, "0")}:00</div>
              )}
              <div className="day-hour-slot">
                {slot.map((e, i) => {
                  const frac = (e.time.getMinutes() + e.time.getSeconds() / 60) / 60;
                  const style = detailed ? { top: `${(frac * 100).toFixed(3)}%` } : undefined;
                  let chipCls = "day-chip";
                  if (onClick && e.id) chipCls += " clickable";
                  if (e.snoozed) chipCls += " snoozed";
                  return (
                    <div
                      className={chipCls}
                      style={style}
                      title={e.label}
                      key={i}
                      onClick={() => e.id && onClick?.(e.id)}
                    >
                      <span className="day-chip-time">
                        {String(e.time.getHours()).padStart(2, "0")}:{String(e.time.getMinutes()).padStart(2, "0")}
                      </span>
                      <span className="day-chip-label">{e.label}</span>
                      {e.flagCount > 0 && <span className="day-chip-flag">⚑{e.flagCount}</span>}
                    </div>
                  );
                })}
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
