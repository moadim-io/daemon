/** Workbench-retention (TTL) helpers shared by the routine form and table row. */

/** Preset (seconds-as-string, label) pairs for the TTL quick-pick buttons. */
export const TTL_PRESETS: [string, string][] = [
  ["3600", "1h"],
  ["86400", "1d"],
  ["604800", "7d"],
  ["2592000", "30d"],
];

/** Render a TTL in seconds as the largest whole unit that divides it evenly. */
export function formatTtl(ttlSecs: number | null | undefined): string {
  if (ttlSecs == null) return "default";
  if (ttlSecs === 0) return "0s";
  if (ttlSecs % 86_400 === 0) return `${ttlSecs / 86_400}d`;
  if (ttlSecs % 3_600 === 0) return `${ttlSecs / 3_600}h`;
  if (ttlSecs % 60 === 0) return `${ttlSecs / 60}m`;
  return `${ttlSecs}s`;
}
