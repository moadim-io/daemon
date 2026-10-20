import { abstime } from "../../lib/cronUtils";
import type { HistogramBucket, RunsHistogram as Histogram } from "./histogramMath";

/** Bottom-to-top stacking order: failures sit on the baseline so they read first. */
const STACK = ["failed", "success", "running", "unknown"] as const;

export interface RunsHistogramProps {
  histogram: Histogram;
  /** Start of the currently selected bucket, if any. */
  selected: number | null;
  onSelect: (bucket: HistogramBucket | null) => void;
}

function bucketLabel(b: HistogramBucket): string {
  const parts = STACK.filter((s) => b.counts[s] > 0).map((s) => `${b.counts[s]} ${s}`);
  return `${abstime(b.from)} – ${abstime(b.to)}: ${b.total === 0 ? "no runs" : parts.join(", ")}`;
}

/**
 * Status-stacked run-volume histogram over time — the timeline strip log explorers and CI
 * dashboards put above their result list. Each bar is a button: clicking it narrows the list
 * below to that time bucket; clicking the selected bar again clears it.
 */
export function RunsHistogram({ histogram, selected, onSelect }: RunsHistogramProps) {
  const { buckets, max } = histogram;
  if (buckets.length === 0) return null;
  const first = buckets[0]!;
  return (
    <div className="runs-hist" role="group" aria-label="Run activity over time">
      <div className="runs-hist-bars">
        {buckets.map((b) => {
          const isSel = selected === b.from;
          const label = bucketLabel(b);
          return (
            <button
              key={b.from}
              type="button"
              className={isSel ? "runs-hist-bar selected" : "runs-hist-bar"}
              aria-pressed={isSel}
              aria-label={label}
              title={label}
              disabled={b.total === 0 && !isSel}
              onClick={() => onSelect(isSel ? null : b)}
            >
              <span className="runs-hist-stack" style={{ height: `${max === 0 ? 0 : (b.total / max) * 100}%` }}>
                {STACK.filter((s) => b.counts[s] > 0).map((s) => (
                  <span key={s} className={`runs-hist-seg ${s}`} style={{ flexGrow: b.counts[s] }} />
                ))}
              </span>
            </button>
          );
        })}
      </div>
      <div className="runs-hist-axis">
        <span>{abstime(first.from)}</span>
        <span>peak {max}/bar</span>
        <span>now</span>
      </div>
    </div>
  );
}
