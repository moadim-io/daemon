/**
 * Chorded keyboard navigation: press `g` then a letter to jump straight to a
 * page, the same "go-to" pattern GitHub, Linear, and Gmail use so power users
 * never have to reach for the mouse. A single source of truth here backs both
 * the Shell's key handler and the `?` shortcuts cheat sheet, so the bindings
 * and their documentation can't drift apart.
 */

export interface NavChord {
  /** The letter pressed after `g` (lowercase). */
  key: string;
  route: string;
  label: string;
}

export const NAV_CHORDS: NavChord[] = [
  { key: "o", route: "/", label: "Overview" },
  { key: "r", route: "/routines", label: "Routines" },
  { key: "h", route: "/heatmap", label: "Heatmap" },
  { key: "l", route: "/reliability", label: "Reliability" },
  { key: "m", route: "/machines", label: "Machines" },
  { key: "s", route: "/settings", label: "Settings" },
];

/** The route a `g <key>` chord jumps to, or `undefined` if `key` isn't bound. */
export function routeForChordKey(key: string): string | undefined {
  return NAV_CHORDS.find((chord) => chord.key === key.toLowerCase())?.route;
}

/**
 * True when `target` is a text-entry element. Bare-letter shortcuts (`g`,
 * `?`) must not fire while the user is typing into a field — checked before
 * every such handler runs.
 */
export function isEditableTarget(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  const tag = target.tagName;
  if (tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT") return true;
  // `closest` (rather than just checking `target` itself) matches the spec's
  // inheritance: a contenteditable region makes every descendant editable too.
  return target.closest('[contenteditable]:not([contenteditable="false"])') !== null;
}
