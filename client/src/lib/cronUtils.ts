import { toString as cronToString } from "cronstrue";

/**
 * Normalizes a 7-field (sec min hour dom month dow year) cron expression to
 * the 5-field form the server understands, matching `croner`'s normalization
 * on the Rust side (`ui/src/cron_utils.rs`). `@`-prefixed shorthand (e.g.
 * `@daily`) and already-5/6-field expressions pass through unchanged.
 */
export function normalizeCron(expr: string): string {
  const s = expr.trim();
  if (s === "" || s.startsWith("@")) return s;
  const parts = s.split(/\s+/);
  return parts.length === 7 ? parts.slice(1, 6).join(" ") : s;
}

/** Returns `[isValid, humanDescription]` for a cron expression. */
export function describeCronLive(expr: string): [boolean, string] {
  if (expr.trim() === "") return [false, "— enter a cron expression —"];
  try {
    return [true, cronToString(normalizeCron(expr), { throwExceptionOnParseError: true })];
  } catch {
    return [false, "Invalid cron expression"];
  }
}

/** Relative-time label for a unix-seconds timestamp: "just now", "5m ago", "3h ago", "2d ago". */
export function reltime(ts: number): string {
  if (ts === 0) return "—";
  const now = Math.floor(Date.now() / 1000);
  const diff = Math.max(0, now - ts);
  if (diff < 60) return "just now";
  if (diff < 3_600) return `${Math.floor(diff / 60)}m ago`;
  if (diff < 86_400) return `${Math.floor(diff / 3_600)}h ago`;
  return `${Math.floor(diff / 86_400)}d ago`;
}

const MONTHS_ABBR = [
  "Jan",
  "Feb",
  "Mar",
  "Apr",
  "May",
  "Jun",
  "Jul",
  "Aug",
  "Sep",
  "Oct",
  "Nov",
  "Dec",
] as const;

/**
 * Absolute local (browser timezone) rendering of a unix-seconds timestamp, e.g.
 * "Jun 21, 2026 12:00". Meant as a tooltip companion to `reltime`'s relative "N ago" text, so
 * hovering reveals wall-clock time. Matches `ui/src/cron_utils.rs`'s `abstime`.
 */
export function abstime(ts: number): string {
  if (ts === 0) return "—";
  const d = new Date(ts * 1000);
  if (Number.isNaN(d.getTime())) return "—";
  const day = String(d.getDate()).padStart(2, "0");
  const hm = `${String(d.getHours()).padStart(2, "0")}:${String(d.getMinutes()).padStart(2, "0")}`;
  return `${MONTHS_ABBR[d.getMonth()]} ${day}, ${d.getFullYear()} ${hm}`;
}
