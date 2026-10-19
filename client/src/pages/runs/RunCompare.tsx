import { useMemo, useState } from "react";
import type { RunSummary } from "../../api/hooks";
import { useRunLog } from "../../api/hooks";
import { reltime } from "../../lib/cronUtils";
import { runStatusLabel } from "../../lib/runDisplay";
import { diffCounts, diffLogs } from "./logDiff";

export interface RunCompareProps {
  routineId: string;
  /** The run currently open on the page — always the diff's "new" side. */
  current: RunSummary;
  /** Every other run of this routine, newest-first, to pick a "compare against" target from. */
  candidates: RunSummary[];
}

/** The most recent `success` run older than `current`, or `undefined` if there is none. */
function lastSuccessBefore(current: RunSummary, candidates: RunSummary[]): RunSummary | undefined {
  return candidates.find((r) => r.status === "success" && r.started_at < current.started_at);
}

/**
 * Run-detail "compare against" panel: diffs this run's log against another run of the same
 * routine (defaulting to the last known-good run), the way CI tools let you diff a failing build
 * against the last green one to spot exactly what changed. Pure client-side line diff over
 * already-fetched log text — no new endpoint.
 */
export function RunCompare({ routineId, current, candidates }: RunCompareProps) {
  const defaultTarget = lastSuccessBefore(current, candidates) ?? candidates[0];
  const [open, setOpen] = useState(false);
  const [targetWorkbench, setTargetWorkbench] = useState<string | undefined>(defaultTarget?.workbench);

  const target = candidates.find((r) => r.workbench === targetWorkbench);
  const currentLog = useRunLog(routineId, current.workbench, open);
  const targetLog = useRunLog(routineId, targetWorkbench ?? "", open && targetWorkbench !== undefined);

  const ops = useMemo(() => {
    if (currentLog.data === undefined || targetLog.data === undefined) return undefined;
    return diffLogs(targetLog.data, currentLog.data);
  }, [currentLog.data, targetLog.data]);

  if (candidates.length === 0) return null;

  return (
    <div className="run-compare">
      <div className="section-acts">
        <button type="button" className="btn btn-ghost btn-sm" onClick={() => setOpen((v) => !v)}>
          {open ? "▾" : "▸"} Compare against another run
        </button>
        {open && (
          <select
            className="input input-sm"
            value={targetWorkbench ?? ""}
            onChange={(e) => setTargetWorkbench(e.target.value || undefined)}
            aria-label="Compare against"
          >
            {candidates.map((r) => (
              <option key={r.workbench} value={r.workbench}>
                {runStatusLabel(r.status)} · {reltime(r.started_at)}
              </option>
            ))}
          </select>
        )}
      </div>

      {open && target !== undefined && (
        <div className="run-compare-body">
          {currentLog.isLoading || targetLog.isLoading ? (
            <div className="empty">
              <div className="spinner" />
            </div>
          ) : currentLog.isError || targetLog.isError ? (
            <div className="logs-error">
              Error: {(currentLog.error ?? targetLog.error)?.message}
            </div>
          ) : ops !== undefined ? (
            <>
              <div className="run-compare-summary">
                {(() => {
                  const { added, removed } = diffCounts(ops);
                  return (
                    <>
                      <span className="c-accent">+{added}</span> <span className="c-red">-{removed}</span> vs{" "}
                      {runStatusLabel(target.status)} run at {reltime(target.started_at)}
                    </>
                  );
                })()}
              </div>
              <div className="logs-wrap">
                {ops.length === 0 ? (
                  <div className="logs-empty">— identical logs —</div>
                ) : (
                  <div className="log-lines">
                    {ops.map((op, i) => (
                      <div key={i} className={`log-line run-diff-${op.kind}`}>
                        <span className="run-diff-marker">
                          {op.kind === "add" ? "+" : op.kind === "remove" ? "-" : " "}
                        </span>
                        <span className="log-lc">{op.line}</span>
                      </div>
                    ))}
                  </div>
                )}
              </div>
            </>
          ) : null}
        </div>
      )}
    </div>
  );
}
