import type { FleetRunSummary } from "../../api/hooks";
import { reltime } from "../../lib/cronUtils";
import { runStatusLabel } from "../../lib/runDisplay";

const TICK_W = 6;
const TICK_GAP = 2;
const TICK_H = 14;

const FILL: Record<FleetRunSummary["status"], string> = {
  running: "var(--warn)",
  success: "var(--ok)",
  failed: "var(--err)",
  unknown: "var(--text-faint)",
};

export interface RunHistorySparklineProps {
  /** This routine's recent runs, oldest to newest (as produced by `groupRecentRuns`). */
  runs: FleetRunSummary[];
}

/** Inline-SVG strip of per-run ticks, colour-coded by outcome. */
export function RunHistorySparkline({ runs }: RunHistorySparklineProps) {
  if (runs.length === 0) {
    return <span className="spark-empty muted">—</span>;
  }
  const width = runs.length * TICK_W - TICK_GAP;
  return (
    <svg
      className="spark"
      role="img"
      aria-label={`Last ${runs.length} runs`}
      width={width}
      height={TICK_H}
      viewBox={`0 0 ${width} ${TICK_H}`}
    >
      {runs.map((run, i) => (
        <rect
          key={`${run.workbench}-${i}`}
          x={i * TICK_W}
          y={0}
          width={TICK_W - TICK_GAP}
          height={TICK_H}
          rx={1}
          fill={FILL[run.status]}
          className={`spark-tick ${run.status}`}
        >
          <title>{`${runStatusLabel(run.status)} · ${reltime(run.started_at)}`}</title>
        </rect>
      ))}
    </svg>
  );
}
