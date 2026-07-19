import { useState } from "react";
import { icalFeedUrl, type RoutineResponse } from "../../api/hooks";
import { CAL_MONTHS, GRID_CELLS, WEEKDAYS, monthStart, occurrencesPerDay } from "../../lib/schedule";
import { isRoutineSnoozed } from "./filter";
import { useToasts } from "../../shell/toasts";

export interface RoutineCalendarProps {
  routines: RoutineResponse[];
  loading: boolean;
  onEdit: (id: string) => void;
}

interface Hit {
  id: string;
  title: string;
  count: number;
  snoozed: boolean;
}

/** Month-calendar view of upcoming routine fire times. */
export function RoutineCalendar({ routines, loading, onEdit }: RoutineCalendarProps) {
  const [offset, setOffset] = useState(0);
  const { addToast } = useToasts();

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
    const counts = occurrencesPerDay(r.schedule, gridStart);
    if (counts === undefined) continue;
    scheduled++;
    const snoozed = isRoutineSnoozed(r, calNow);
    counts.forEach((c, i) => {
      if (c > 0) cells[i]?.push({ id: r.id, title: r.title, count: c, snoozed });
    });
  }

  const monthLabel = `${CAL_MONTHS[first.getMonth()]} ${first.getFullYear()}`;

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
              let cls = "cal-day";
              if (date.getMonth() !== first.getMonth()) cls += " other-month";
              if (date.getTime() === today.getTime()) cls += " today";
              return (
                <div className={cls} key={i}>
                  <div className="cal-daynum">{date.getDate()}</div>
                  <div className="cal-hits">
                    {hits.slice(0, 4).map((hit, idx) => {
                      const label = hit.count > 1 ? `${hit.title} ×${hit.count}` : hit.title;
                      let chipCls = "cal-chip clickable";
                      if (hit.snoozed) chipCls += " snoozed";
                      return (
                        <div
                          className={chipCls}
                          title={label}
                          key={`${hit.id}-${idx}`}
                          onClick={() => onEdit(hit.id)}
                        >
                          {label}
                        </div>
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
    </div>
  );
}
