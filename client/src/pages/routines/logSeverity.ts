/**
 * Line-level severity triage for the log viewer: flags lines that look like errors or warnings so
 * the viewer can color them, count them, filter to them and jump between them.
 */

export type Severity = "error" | "warn";

// ponytail: keyword heuristic — "0 errors" still reads as an error line. Swap for structured
// levels if the daemon ever ships them.
const ERROR_RE = /\b(errors?|fatal|panic(?:ked)?|exception|traceback|failed|failure)\b/i;
const WARN_RE = /\b(warn(?:ing|ings)?|deprecated)\b/i;

/** `"error"`, `"warn"`, or `undefined` for a plain line. Error wins when both match. */
export function severityOf(line: string): Severity | undefined {
  if (ERROR_RE.test(line)) return "error";
  if (WARN_RE.test(line)) return "warn";
  return undefined;
}

/** Counts of error and warning lines. */
export function severityCounts(lines: readonly string[]): { error: number; warn: number } {
  const out = { error: 0, warn: 0 };
  for (const l of lines) {
    const s = severityOf(l);
    if (s) out[s]++;
  }
  return out;
}

/**
 * Next (`dir = 1`) or previous (`dir = -1`) problem line number relative to `current`, wrapping
 * around. `problems` must be ascending. `undefined` when there are none.
 */
export function stepProblem(problems: readonly number[], current: number | undefined, dir: 1 | -1): number | undefined {
  if (problems.length === 0) return undefined;
  if (dir === 1) {
    return problems.find((ln) => current === undefined || ln > current) ?? problems[0];
  }
  for (let i = problems.length - 1; i >= 0; i--) {
    const ln = problems[i] as number;
    if (current === undefined || ln < current) return ln;
  }
  return problems[problems.length - 1];
}
