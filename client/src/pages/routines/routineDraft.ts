/**
 * Pure text <-> structured-field helpers for the routine create/edit form. Direct port of the
 * free-function helpers in `ui/src/routines/form.rs (removed)`.
 */
import type { components } from "../../api/schema.gen";

type Repository = components["schemas"]["Repository"];

const CLONE_PREFIX = "Copy of ";

/** Prefixes `title` with "Copy of " unless it already carries the prefix (no doubling up). */
export function cloneTitle(title: string): string {
  return title.startsWith(CLONE_PREFIX) ? title : `${CLONE_PREFIX}${title}`;
}

/** One repository per line, as `"{url} {branch} [auto_pull=false]"` (branch omitted when unset). */
export function reposToText(repos: Repository[]): string {
  return repos
    .map((r) => {
      const parts = [r.repository];
      if (r.branch && r.branch.trim() !== "") parts.push(r.branch);
      if (r.auto_pull === false) parts.push("auto_pull=false");
      return parts.join(" ");
    })
    .join("\n");
}

/** Parses the repositories textarea: first token = url, optional branch, optional `auto_pull=false`. */
export function textToRepos(text: string): Repository[] {
  const out: Repository[] = [];
  for (const line of text.split("\n")) {
    const tokens = line.trim().split(/\s+/).filter(Boolean);
    const repository = tokens.shift();
    if (!repository) continue;
    const autoPullTokenIndex = tokens.findIndex((token) => token === "auto_pull=false" || token === "auto-pull=false");
    const auto_pull = autoPullTokenIndex === -1;
    if (autoPullTokenIndex !== -1) tokens.splice(autoPullTokenIndex, 1);
    out.push({ repository, branch: tokens[0] ?? null, auto_pull });
  }
  return out;
}

export function tagsToText(tags: string[]): string {
  return tags.join(", ");
}

export function textToTags(text: string): string[] {
  return text
    .split(",")
    .map((t) => t.trim())
    .filter((t) => t !== "");
}

/** Blank or non-numeric input both mean "use the server default" (mirrors Rust's `u64::parse`). */
export function parseTtl(raw: string): number | null {
  const trimmed = raw.trim();
  if (trimmed === "" || !/^\d+$/.test(trimmed)) return null;
  return Number(trimmed);
}
