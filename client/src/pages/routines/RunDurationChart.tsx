import type { RunSummary } from "../../api/hooks";
import { abstime, reltime } from "../../lib/cronUtils";
import { fmtRunDuration, runStatusLabel } from "../../lib/runDisplay";
import { buildDurationBars } from "./durationChart";

export interface RunDurationChartProps {
  /** Newest-first, matching the history table rendered alongside it. */
  runs: RunSummary[];
  selected: string | undefined;
  onSelect: (workbench: string) => void;
}

/** Horizontal duration-bar chart for a routine's run history: spot slow outliers and failure clusters at a glance. */
export function RunDurationChart({ runs, selected, onSelect }: RunDurationChartProps) {
  if (runs.length === 0) return null;
  const bars = buildDurationBars(runs);

  return (
    <div className="duration-chart" role="list" aria-label="Run duration chart">
      {bars.map((bar) => {
        const durationText = bar.durationSecs === null ? "in progress" : fmtRunDuration(0, bar.durationSecs);
        const isSelected = selected === bar.workbench;
        return (
          <button
            type="button"
            key={bar.workbench}
            role="listitem"
            className={`duration-row${isSelected ? " row-selected" : ""}`}
            title={`${runStatusLabel(bar.status)} · ${durationText} · ${abstime(bar.startedAt)}${
              bar.exitCode != null ? ` · exit ${bar.exitCode}` : ""
            }`}
            onClick={() => onSelect(bar.workbench)}
          >
            <span className="duration-row-time">{reltime(bar.startedAt)}</span>
            <span className="duration-row-track">
              <span className={`duration-bar ${bar.status}`} style={{ width: `${bar.widthPct}%` }} />
            </span>
            <span className="duration-row-dur">{durationText}</span>
          </button>
        );
      })}
    </div>
  );
}
