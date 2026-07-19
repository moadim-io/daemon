export type RView = "table" | "calendar" | "day";

export interface ViewToggleProps {
  view: RView;
  onSetView: (v: RView) => void;
}

const VIEWS: [RView, string][] = [
  ["table", "LIST"],
  ["calendar", "CALENDAR"],
  ["day", "DAY"],
];

/** List / Calendar / Day view switcher. */
export function ViewToggle({ view, onSetView }: ViewToggleProps) {
  return (
    <div className="view-toggle">
      {VIEWS.map(([v, label]) => (
        <button
          key={v}
          type="button"
          className={view === v ? "view-btn active" : "view-btn"}
          onClick={() => onSetView(v)}
        >
          {label}
        </button>
      ))}
    </div>
  );
}
