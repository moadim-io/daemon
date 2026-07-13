/**
 * Pure text-search helpers backing the log viewer's search box: filtering, match counting, and
 * Unicode-safe highlighting. Direct port of the pure functions in `ui/src/log_viewer.rs`.
 */

/**
 * Splits like Rust's `str::lines()`: empty input yields zero lines, and a trailing `\n` doesn't
 * produce a phantom empty final line (unlike a bare `String.split("\n")`).
 */
function linesOf(content: string): string[] {
  if (content === "") return [];
  const parts = content.split("\n");
  if (parts.length > 0 && parts[parts.length - 1] === "") parts.pop();
  return parts;
}

/** `(1-based line number, line text)` for every line matching `query` (case-insensitive). */
export function filterLines(content: string, query: string): [number, string][] {
  const needle = query.trim().toLowerCase();
  const out: [number, string][] = [];
  const lines = linesOf(content);
  for (let i = 0; i < lines.length; i++) {
    const line = lines[i] ?? "";
    if (needle === "" || line.toLowerCase().includes(needle)) out.push([i + 1, line]);
  }
  return out;
}

/** `(matching lines, total lines)` for `query` (case-insensitive). Blank query matches every line. */
export function matchCount(content: string, query: string): [number, number] {
  const lines = linesOf(content);
  const total = lines.length;
  const needle = query.trim().toLowerCase();
  if (needle === "") return [total, total];
  const hits = lines.filter((l) => l.toLowerCase().includes(needle)).length;
  return [hits, total];
}

/**
 * Splits `text` into alternating `(isMatch, slice)` segments for case-insensitive highlighting of
 * `query` occurrences. Walks per-code-point (via `Array.from`) rather than reusing byte/UTF-16
 * offsets from a lowercased copy, so a locale's one-to-many lowercase expansions (e.g. German
 * "ẞ" → "ss") can't misalign a highlighted slice against the original text.
 */
export function highlightSegments(text: string, query: string): [boolean, string][] {
  const needle = query.trim().toLowerCase();
  if (needle === "") return [[false, text]];

  const chars = Array.from(text);
  const lower = chars.map((c) => c.toLowerCase());
  const needleChars = Array.from(needle);
  const n = needleChars.length;

  const segments: [boolean, string][] = [];
  let i = 0;
  let gapStart = 0;
  while (i < chars.length) {
    const window = lower.slice(i, i + n).join("");
    if (window === needle && window.length > 0) {
      if (i > gapStart) segments.push([false, chars.slice(gapStart, i).join("")]);
      segments.push([true, chars.slice(i, i + n).join("")]);
      i += n;
      gapStart = i;
    } else {
      i++;
    }
  }
  if (gapStart < chars.length) segments.push([false, chars.slice(gapStart).join("")]);
  return segments;
}
