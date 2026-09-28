/**
 * Failure causes for the RELIABILITY page: every `failed` run in the fetched sample grouped by
 * its exit code, with a plain-language meaning for well-known codes, the routines it hit and the
 * newest occurrence.
 *
 * Best practice (cron monitors — Better Stack, Cronitor, Healthchecks.io; error trackers like
 * Sentry grouping by signature): categorize failures by exit code so an operator sees *why* runs
 * fail, not just that they do — e.g. 137s across many routines point at host memory pressure
 * rather than one routine's bug.
 */
import type { FleetRunSummary } from "../../api/hooks";

const KNOWN: Record<number, string> = {
  1: "general error",
  2: "misuse / bad arguments",
  124: "timed out",
  126: "not executable",
  127: "command not found",
  130: "interrupted (SIGINT)",
  137: "killed (SIGKILL / OOM)",
  143: "terminated (SIGTERM)",
};

/** Human meaning of an exit code; `null` = no code recorded. */
export function exitCodeMeaning(code: number | null): string {
  if (code === null) return "no exit code recorded";
  const known = KNOWN[code];
  if (known) return known;
  if (code > 128 && code < 160) return `signal ${code - 128}`;
  return "routine-specific";
}

export interface FailureCause {
  code: number | null;
  count: number;
  /** Affected routines, most failures first. */
  routines: { id: string; title: string; count: number }[];
  /** Newest failed run with this code. */
  last: { routineId: string; workbench: string; at: number };
}

export interface FailureCauseReport {
  causes: FailureCause[];
  totalFailed: number;
}

/** Groups failed runs (any order) by exit code, most frequent first (ties: newest first). */
export function computeFailureCauses(runs: FleetRunSummary[]): FailureCauseReport {
  const byCode = new Map<number | null, FailureCause>();
  let totalFailed = 0;
  for (const run of runs) {
    if (run.status !== "failed") continue;
    totalFailed += 1;
    const code = run.exit_code ?? null;
    const at = run.finished_at ?? run.started_at;
    let cause = byCode.get(code);
    if (!cause) {
      cause = { code, count: 0, routines: [], last: { routineId: run.routine_id, workbench: run.workbench, at } };
      byCode.set(code, cause);
    }
    cause.count += 1;
    if (at > cause.last.at) cause.last = { routineId: run.routine_id, workbench: run.workbench, at };
    const r = cause.routines.find((x) => x.id === run.routine_id);
    if (r) r.count += 1;
    else cause.routines.push({ id: run.routine_id, title: run.routine_title, count: 1 });
  }
  const causes = [...byCode.values()];
  for (const c of causes) c.routines.sort((a, b) => b.count - a.count);
  causes.sort((a, b) => b.count - a.count || b.last.at - a.last.at);
  return { causes, totalFailed };
}
