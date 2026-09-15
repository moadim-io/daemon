/**
 * Pure line-based diff backing the run-detail "Compare" view: given two runs' raw log text,
 * shows what changed between them the way a CI tool's "diff against last green build" view does
 * — spot the one line that changed without re-reading the whole log top to bottom.
 */
import { linesOf } from "../routines/logSearch";

export type DiffOp =
  | { kind: "equal"; line: string }
  | { kind: "add"; line: string }
  | { kind: "remove"; line: string };

/** Above this many lines per side, we skip the O(n*m) LCS table and fall back to a flat diff. */
const LCS_LINE_CAP = 4000;

/** Longest-common-subsequence table, `table[i][j]` = LCS length of `a[0..i)` and `b[0..j)`. */
function lcsTable(a: string[], b: string[]): number[][] {
  const table: number[][] = Array.from({ length: a.length + 1 }, () => new Array(b.length + 1).fill(0));
  for (let i = a.length - 1; i >= 0; i--) {
    for (let j = b.length - 1; j >= 0; j--) {
      table[i]![j] = a[i] === b[j] ? table[i + 1]![j + 1]! + 1 : Math.max(table[i + 1]![j]!, table[i]![j + 1]!);
    }
  }
  return table;
}

/** Walks the LCS table to emit a minimal equal/add/remove sequence turning `a` into `b`. */
function walkLcs(a: string[], b: string[], table: number[][]): DiffOp[] {
  const ops: DiffOp[] = [];
  let i = 0;
  let j = 0;
  while (i < a.length && j < b.length) {
    if (a[i] === b[j]) {
      ops.push({ kind: "equal", line: a[i]! });
      i++;
      j++;
    } else if (table[i + 1]![j]! >= table[i]![j + 1]!) {
      ops.push({ kind: "remove", line: a[i]! });
      i++;
    } else {
      ops.push({ kind: "add", line: b[j]! });
      j++;
    }
  }
  while (i < a.length) ops.push({ kind: "remove", line: a[i++]! });
  while (j < b.length) ops.push({ kind: "add", line: b[j++]! });
  return ops;
}

// ponytail: full LCS is O(n*m); past LCS_LINE_CAP we skip alignment and just show both logs as a
// straight remove-all/add-all pair. Upgrade to a chunked/Myers diff if huge-log comparison matters.
function flatDiff(a: string[], b: string[]): DiffOp[] {
  return [...a.map((line): DiffOp => ({ kind: "remove", line })), ...b.map((line): DiffOp => ({ kind: "add", line }))];
}

/** Line-by-line diff of two log texts, oldest→newest ops order. */
export function diffLogs(oldContent: string, newContent: string): DiffOp[] {
  const a = linesOf(oldContent);
  const b = linesOf(newContent);
  if (a.length > LCS_LINE_CAP || b.length > LCS_LINE_CAP) return flatDiff(a, b);
  return walkLcs(a, b, lcsTable(a, b));
}

/** `{ added, removed }` line counts for a diff — the compact summary shown before expanding it. */
export function diffCounts(ops: DiffOp[]): { added: number; removed: number } {
  let added = 0;
  let removed = 0;
  for (const op of ops) {
    if (op.kind === "add") added++;
    else if (op.kind === "remove") removed++;
  }
  return { added, removed };
}
