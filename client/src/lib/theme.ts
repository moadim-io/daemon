/**
 * localStorage key for the theme preference. Distinct from `ui/`'s
 * `moadim.theme` so the two independently-themed UIs never fight over the
 * same stored value while both are served.
 */
const THEME_KEY = "moadim.client.theme";

/** Reads the persisted theme from localStorage. Returns `true` for light theme. */
export function loadThemeLight(): boolean {
  try {
    return localStorage.getItem(THEME_KEY) === "light";
  } catch {
    return false;
  }
}

/** Persists the theme choice to localStorage (best-effort; ignores storage errors). */
export function saveThemeLight(light: boolean): void {
  try {
    localStorage.setItem(THEME_KEY, light ? "light" : "dark");
  } catch {
    // storage unavailable (private mode / quota) — in-memory choice still applies
  }
}

/** Applies or removes the `theme-light` CSS class on `<html>`. */
export function applyTheme(light: boolean): void {
  document.documentElement.classList.toggle("theme-light", light);
}
