---
"moadim": patch
---

fix(client): restore a working `client/` build (broken on `main`, `client (vitest)` CI red for 3+ pushes)

Two independent, pre-existing dependency breaks, surfaced while making the React client the sole
UI (see the "remove the legacy Yew UI" changeset):

- `@vitejs/plugin-react@6.0.3` (bumped in #1191) peer-requires `vite@^8.0.0`, but this repo pins
  `vite@^6.0.5`. Vite's own package no longer exposes the `./internal` subpath 6.0.3 imports,
  so `vite build`/`vitest` failed to even load `vite.config.ts`. Downgraded to `@vitejs/plugin-react@^5.2.0`,
  the latest release still compatible with `vite ^6`.
- `react-dom` was bumped to `^19.2.7` in #1187 without bumping `react` itself, which stayed at
  `^18.3.1` — a cross-major mismatch that crashes on mount (`Cannot read properties of undefined
  (reading 'S')`) the moment the vite/plugin-react fix above let tests actually run. Bumped `react`
  and `@types/react` to `^19.2.7`/`^19.2.3` to match. React 19's stricter `RefObject<T>` typing
  (no longer implicitly nullable) surfaced one real type error: `FilterBarProps.searchRef` is now
  `RefObject<HTMLInputElement | null>`, matching what `useRef<HTMLInputElement>(null)` actually
  returns.

No intentional behavior change; `pnpm --filter client build/typecheck/lint/test` are all green
again.
