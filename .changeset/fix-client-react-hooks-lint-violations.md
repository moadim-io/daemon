---
"moadim": patch
---

fix(client): resolve `client-lint`'s 8 `react-hooks/purity` and `react-hooks/set-state-in-effect` errors

The `eslint-plugin-react-hooks` bump to 7.1.1 (#1185) enabled stricter React
Compiler rules that flag pre-existing code: four `Date.now()` calls during
render (`react-hooks/purity`) and four `setState` calls synchronously inside
a `useEffect` (`react-hooks/set-state-in-effect`). This left `pnpm --filter
client lint` — part of both CI's `client-lint` job and the local pre-push
hook — failing on `main` for any contributor who runs it, independent of
what their own change touches.

- Added a shared `useNow()` hook (`client/src/lib/useNow.ts`) that reads the
  clock inside a timer effect instead of during render, and reused it in
  `RefreshControl`, `RoutineFlags`, and `RoutineHistory` (which previously
  each read `Date.now()` directly in their render body).
- Moved four `setState` calls (`RoutinesPage`'s deep-link page and stale-selection
  prune, `SettingsPage`'s draft-seeding, `CommandPalette`'s reset-on-open) out
  of `useEffect` and into a lazy `useState` initializer or a guarded
  render-time update, per React's own "Adjusting state when a prop changes"
  guidance — no behavior change intended.

No dependency versions changed here; unrelated to #1251, which fixes a
separate `@vitejs/plugin-react`/`vite` peer-dependency break that currently
also blocks `pnpm --filter client test` from even starting.
