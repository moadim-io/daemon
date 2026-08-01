import { useEffect, useState } from "react";
import { icalFeedUrl, type RoutineResponse } from "../../api/hooks";
import {
  CAL_MONTHS,
  GRID_CELLS,
  WEEKDAYS,
  dateOnly,
  fireTimesOnDay,
  monthStart,
  occurrencesPerDay,
  scheduleList,
} from "../../lib/schedule";
import { isRoutineSnoozed } from "./filter";
import { useToasts } from "../../shell/toasts";

export interface RoutineCalendarProps {
  routines: RoutineResponse[];
  loading: boolean;
  onEdit: (id: string) => void;
  onTrigger: (id: string) => void;
}

interface Hit {
  id: string;
  title: string;
  count: number;
  snoozed: boolean;
}

interface DayFire {
  id: string;
  title: string;
  times: Date[];
  snoozed: boolean;
}

function fmtDateLabel(day: Date): string {
  return day.toLocaleDateString(undefined, { weekday: "short", month: "short", day: "numeric", year: "numeric" });
}

function fmtHm(dt: Date): string {
  return `${String(dt.getHours()).padStart(2, "0")}:${String(dt.getMinutes()).padStart(2, "0")}`;
}

function firesForDay(routines: RoutineResponse[], day: Date, now: Date): DayFire[] {
  return routines
    .filter((r) => r.enabled)
    .map((r) => {
      const seen = new Set<number>();
      const times = scheduleList(r)
        .flatMap((schedule) => fireTimesOnDay(schedule, day))
        .sort((a, b) => a.getTime() - b.getTime())
        .filter((fire) => {
          const ms = fire.getTime();
          if (seen.has(ms)) return false;
          seen.add(ms);
          return true;
        });
      return { id: r.id, title: r.title, times, snoozed: isRoutineSnoozed(r, now) };
    })
    .filter((fire) => fire.times.length > 0)
    .sort((a, b) => (a.times[0]?.getTime() ?? 0) - (b.times[0]?.getTime() ?? 0) || a.title.localeCompare(b.title));
}

/** Month-calendar view of upcoming routine fire times. */
export function RoutineCalendar({ routines, loading, onEdit, onTrigger }: RoutineCalendarProps) {
  const [offset, setOffset] = useState(0);
  const [selectedDay, setSelectedDay] = useState<Date | undefined>(undefined);
  const { addToast } = useToasts();

  useEffect(() => {
    if (selectedDay === undefined) return;
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") setSelectedDay(undefined);
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [selectedDay]);

  if (loading) {
    return (
      <div className="table-wrap">
        <div className="empty">
          <div className="spinner" />
        </div>
      </div>
    );
  }

  const onSubscribe = () => {
    const url = `${window.location.origin}${icalFeedUrl()}`;
    navigator.clipboard
      .writeText(url)
      .then(() => addToast("Calendar feed URL copied", "ok"))
      .catch(() => addToast("Copy failed", "err"));
  };

  const today = new Date();
  today.setHours(0, 0, 0, 0);
  const first = monthStart(today, offset);
  const gridStart = new Date(first);
  gridStart.setDate(gridStart.getDate() - first.getDay());

  const calNow = new Date();
  const cells: Hit[][] = Array.from({ length: GRID_CELLS }, () => []);
  let scheduled = 0;
  for (const r of routines.filter((r) => r.enabled)) {
    const countsBySchedule = scheduleList(r)
      .map((schedule) => occurrencesPerDay(schedule, gridStart))
      .filter((counts): counts is number[] => counts !== undefined);
    if (countsBySchedule.length === 0) continue;
    scheduled++;
    const snoozed = isRoutineSnoozed(r, calNow);
    const counts = countsBySchedule.reduce((acc, cur) => acc.map((v, i) => v + (cur[i] ?? 0)));
    counts.forEach((c, i) => {
      if (c > 0) cells[i]?.push({ id: r.id, title: r.title, count: c, snoozed });
    });
  }

  const monthLabel = `${CAL_MONTHS[first.getMonth()]} ${first.getFullYear()}`;
  const dayFires = selectedDay ? firesForDay(routines, selectedDay, calNow) : [];

  return (
    <div className="cal-wrap">
      <div className="cal-nav">
        <button type="button" className="btn-refresh" title="Previous month" aria-label="Previous month" onClick={() => setOffset((o) => o - 1)}>
          ‹
        </button>
        <div className="cal-month">{monthLabel}</div>
        <button type="button" className="btn-refresh" title="Next month" aria-label="Next month" onClick={() => setOffset((o) => o + 1)}>
          ›
        </button>
        <button type="button" className="btn btn-ghost btn-sm" onClick={() => setOffset(0)}>
          TODAY
        </button>
        <button type="button" className="btn btn-ghost btn-sm" title="Copy the routines.ics feed URL" onClick={onSubscribe}>
          SUBSCRIBE
        </button>
      </div>
      {scheduled === 0 ? (
        <div className="empty">
          <div className="empty-icon">🗓</div>
          <div className="empty-msg">NOTHING SCHEDULED</div>
          <div className="empty-sub">enabled routines with a valid schedule appear here</div>
        </div>
      ) : (
        <>
          <div className="cal-weekdays">
            {WEEKDAYS.map((d) => (
              <div className="cal-weekday" key={d}>
                {d}
              </div>
            ))}
          </div>
          <div className="cal-grid">
            {cells.map((hits, i) => {
              const date = new Date(gridStart);
              date.setDate(date.getDate() + i);
              const day = dateOnly(date);
              let cls = "cal-day";
              if (date.getMonth() !== first.getMonth()) cls += " other-month";
              if (date.getTime() === today.getTime()) cls += " today";
              const dateLabel = fmtDateLabel(day);
              return (
                <div
                  className={cls}
                  key={i}
                  role="button"
                  tabIndex={0}
                  aria-label={`Open schedule details for ${dateLabel}`}
                  onClick={() => setSelectedDay(day)}
                  onKeyDown={(e) => {
                    if (e.key === "Enter" || e.key === " ") {
                      e.preventDefault();
                      setSelectedDay(day);
                    }
                  }}
                >
                  <div className="cal-daynum">{date.getDate()}</div>
                  <div className="cal-hits">
                    {hits.slice(0, 4).map((hit, idx) => {
                      const label = hit.count > 1 ? `${hit.title} ×${hit.count}` : hit.title;
                      let chipCls = "cal-chip clickable";
                      if (hit.snoozed) chipCls += " snoozed";
                      return (
                        <button
                          type="button"
                          className={chipCls}
                          aria-label={`Edit ${hit.title}`}
                          title={label}
                          key={`${hit.id}-${idx}`}
                          onClick={(e) => {
                            e.stopPropagation();
                            onEdit(hit.id);
                          }}
                        >
                          {label}
                        </button>
                      );
                    })}
                    {hits.length > 4 && <div className="cal-more">+{hits.length - 4} more</div>}
                  </div>
                </div>
              );
            })}
          </div>
        </>
      )}

      {selectedDay && (
        <div className="overlay" onClick={() => setSelectedDay(undefined)}>
          <div className="dialog cal-day-dialog" role="dialog" aria-modal="true" aria-label="Calendar day details" onClick={(e) => e.stopPropagation()}>
            <div className="dialog-title">{fmtDateLabel(selectedDay)}</div>
            <div className="dialog-msg">Scheduled routine fires for this local day.</div>
            {dayFires.length === 0 ? (
              <div className="cal-day-empty">No routine fires on this day.</div>
            ) : (
              <div className="cal-day-fire-list">
                {dayFires.map((fire) => (
                  <div className="cal-day-fire" key={fire.id}>
                    <div className="cal-day-fire-main">
                      <div className="cal-day-fire-title">{fire.title}</div>
                      <div className="cal-day-fire-times">
                        {fire.times.map((time) => (
                          <span className="cal-time-chip" key={time.getTime()}>
                            {fmtHm(time)}
                          </span>
                        ))}
                      </div>
                    </div>
                    <button
                      type="button"
                      className="act-btn run"
                      aria-label={`Run ${fire.title} now`}
                      disabled={fire.snoozed}
                      title={fire.snoozed ? "Routine is snoozed" : "Trigger this routine now"}
                      onClick={() => onTrigger(fire.id)}
                    >
                      RUN NOW
                    </button>
                  </div>
                ))}
              </div>
            )}
            <div className="dialog-actions">
              <button type="button" className="btn btn-ghost" onClick={() => setSelectedDay(undefined)}>
                Close
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
