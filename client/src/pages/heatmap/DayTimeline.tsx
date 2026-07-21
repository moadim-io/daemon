import { bucketDayFires, dayTimelineLabel, type TimelineItem } from "./dayTimelineMath";

interface DayTimelineProps {
  day: Date;
  items: TimelineItem[];
  loading: boolean;
  isToday: boolean;
  currentHour: number;
  onPrev: () => void;
  onNext: () => void;
  onToday: () => void;
  onClose: () => void;
}

/** The day drill-down: every fire on `day`, laid out by hour. A simplified,
 * redesigned take on `ui/src/day_timeline.rs (removed)` — same underlying fire-time math
 * (see `dayTimeline.ts`), without that version's pixel-precise zoom levels: each
 * chip already shows its exact HH:MM, so a zoom control isn't needed to read
 * sub-hour timing. */
export function DayTimeline({
  day,
  items,
  loading,
  isToday,
  currentHour,
  onPrev,
  onNext,
  onToday,
  onClose,
}: DayTimelineProps) {
  const buckets = bucketDayFires(items, day);
  const total = buckets.reduce((sum, b) => sum + b.length, 0);

  return (
    <section className="hm-day card">
      <div className="hm-day-nav">
        <button className="btn btn-ghost btn-sm" onClick={onPrev} aria-label="Previous day">
          {"‹"}
        </button>
        <div className="hm-day-title">{dayTimelineLabel(day)}</div>
        <button className="btn btn-ghost btn-sm" onClick={onNext} aria-label="Next day">
          {"›"}
        </button>
        <button className="btn btn-ghost btn-sm" onClick={onToday}>
          TODAY
        </button>
        <div className="hm-day-spacer" />
        <button className="btn btn-ghost btn-sm" onClick={onClose} aria-label="Close day view">
          {"✕"}
        </button>
      </div>

      {loading ? (
        <div className="empty">
          <div className="spinner" />
        </div>
      ) : total === 0 ? (
        <div className="empty">
          <div className="empty-icon">🗓</div>
          <div className="empty-msg">NOTHING SCHEDULED</div>
          <div className="empty-sub">no fire times on this day</div>
        </div>
      ) : (
        <div className="hm-day-hours">
          {buckets.map((bucket, hour) => (
            <div
              key={hour}
              className={
                isToday && hour === currentHour ? "hm-day-hour now" : "hm-day-hour"
              }
            >
              <div className="hm-day-hour-label">{String(hour).padStart(2, "0")}:00</div>
              <div className="hm-day-hour-slot">
                {bucket.map((entry, i) => (
                  <div
                    key={i}
                    className={entry.snoozed ? "hm-day-chip snoozed" : "hm-day-chip"}
                    title={entry.label}
                  >
                    <span className="hm-day-chip-time">
                      {String(entry.time.getHours()).padStart(2, "0")}:
                      {String(entry.time.getMinutes()).padStart(2, "0")}
                    </span>
                    <span className="hm-day-chip-label">{entry.label}</span>
                    {entry.flagCount > 0 && (
                      <span className="hm-day-chip-flag">{`⚑${entry.flagCount}`}</span>
                    )}
                  </div>
                ))}
              </div>
            </div>
          ))}
        </div>
      )}
    </section>
  );
}
