/** Direct port of `humanize_bytes` in `ui/src/routines/model.rs (removed)`. */
const UNITS = ["B", "KB", "MB", "GB", "TB"] as const;

/** Render a byte count as a short human-readable size (1024-based). */
export function humanizeBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  let size = bytes;
  let unit = 0;
  while (size >= 1024 && unit < UNITS.length - 1) {
    size /= 1024;
    unit++;
  }
  return `${size.toFixed(1)} ${UNITS[unit]}`;
}
